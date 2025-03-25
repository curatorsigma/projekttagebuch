use std::sync::Arc;

use askama_axum::IntoResponse;
/// The routes protected by a login
use axum::{
    http::StatusCode,
    routing::{delete, get, post},
    Extension, Router,
};
use tracing::warn;
use uuid::Uuid;

use crate::{
    config::Config,
    db::get_person,
    types::{HasID, Person},
    web_server::InternalServerErrorTemplate,
};

use super::login::AuthSession;

fn error_display(s: &str) -> String {
    // we cannot control hx-swap separately for hx-target and hx-target-error
    // so we swap outer-html and add the surrounding div all the time
    format!("<div class=\"text-red-500 flex justify-center\" id=\"error_display\" _=\"on htmx:beforeSend from elsewhere set my innerHTML to ''\">{}</div>", s)
}

pub(crate) fn create_protected_router() -> Router {
    // todo redo routes
    // we want posts to be in their own /api subdir instead of web
    Router::new()
        .route("/", get(self::get::root))
        .route(
            "/web/project/new",
            get(self::get::project_new_template).post(self::post::project_new),
        )
        .route(
            "/web/project/:project_id/header_only",
            get(self::get::project_header_only),
        )
        .route(
            "/web/project/:project_id/with_users",
            get(self::get::project_with_users),
        )
        .route(
            "/web/project/:project_id/new_member",
            get(self::get::project_new_member_template).post(self::post::project_new_member),
        )
        .route("/web/search_user", post(self::post::search_user_results))
        .route("/web/project/:project_id/remove_member",
            delete(self::delete::project_remove_member))
        .route("/web/project/:project_id/set_member_permission", post(self::post::project_set_member_permission))
}

/// Get the user (as present in db) from the auth session, creating relevant Server Error returns
async fn get_user_from_session(
    auth_session: AuthSession,
    config: Arc<Config>,
) -> Result<Person<HasID>, impl IntoResponse> {
    let user = if let Some(x) = auth_session.user {
        x
    } else {
        let error_uuid = Uuid::new_v4();
        warn!("Sending internal server error because there is no user in the auth session. uuid: {error_uuid}");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            InternalServerErrorTemplate { error_uuid },
        )
            .into_response());
    };

    match get_person(config.pg_pool.clone(), &user.username).await {
        Ok(Some(x)) => Ok(x),
        Ok(None) => {
            let error_uuid = Uuid::new_v4();
            // this should fix itself on the next LDAP->DB sync period
            warn!("Sending internal server error because a logged-in user did not exist. {error_uuid}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                InternalServerErrorTemplate { error_uuid },
            )
                .into_response());
        }
        Err(e) => {
            let error_uuid = Uuid::new_v4();
            warn!("Sending internal Server error because I cannot get a user by name: {e}. {error_uuid}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                InternalServerErrorTemplate { error_uuid },
            )
                .into_response());
        }
    }
}

pub(super) mod get {
    use crate::{
        db::{get_person, get_project, get_projects},
        types::{HasID, Project, UserPermission},
        web_server::{login::AuthSession, InternalServerErrorTemplate},
    };

    use super::*;
    use core::borrow::Borrow;

    use askama_axum::IntoResponse;
    use axum::{extract::Path, http::StatusCode};
    use tracing::{error, info, trace, warn};
    use uuid::Uuid;

    use crate::config::Config;

    #[derive(askama_axum::Template)]
    #[template(path = "landing/complete.html", escape = "none")]
    struct LandingAsUser {
        username: String,
        projects: Vec<Project<HasID>>,
        permission: UserPermission,
    }

    #[derive(askama_axum::Template)]
    #[template(path = "landing/not_yet_synced.html")]
    struct LandingNotYetSynced {
        username: String,
        retry_after: u32,
    }

    pub(super) async fn root(
        auth_session: AuthSession,
        Extension(config): Extension<Arc<Config>>,
    ) -> impl IntoResponse {
        let user = if let Some(x) = auth_session.user {
            x
        } else {
            let error_uuid = Uuid::new_v4();
            warn!("Sending internal server error because there is no user in the auth session. uuid: {error_uuid}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                InternalServerErrorTemplate { error_uuid },
            )
                .into_response();
        };

        // get projects
        let projects = match get_projects(config.pg_pool.clone()).await {
            Ok(x) => x,
            Err(e) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal server error because I cannot get projects from the DB: {e}. Error Code is {error_uuid}.");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                )
                    .into_response();
            }
        };
        let user_obj = match get_person(config.pg_pool.clone(), &user.username).await {
            Ok(x) => x,
            Err(e) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal server error because I cannot get the logged in user from the DB: {e}. Error Code is {error_uuid}.");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                )
                    .into_response();
            }
        };
        match user_obj {
            Some(person) => LandingAsUser {
                username: person.name,
                projects,
                permission: person.global_permission,
            }
            .into_response(),
            // user exists in LDAP but does not yet exist in DB.
            // Tell the user to come back in the right amount of time.
            None => {
                warn!("Sending not_yet_synced template, because user {} logged in successfully but is not yet cached in our db.", user.username);
                LandingNotYetSynced {
                    username: user.username,
                    retry_after: config.user_resync_interval,
                }
                .into_response()
            }
        }
    }

    #[derive(askama_axum::Template, Debug)]
    #[template(path = "project/new.html")]
    struct NewProject {}

    /// return the html form for adding a new project
    pub(super) async fn project_new_template() -> impl IntoResponse {
        NewProject {}.into_response()
    }

    pub(super) async fn project_header_only(
        auth_session: AuthSession,
        Extension(config): Extension<Arc<Config>>,
        Path(project_id): Path<i32>,
    ) -> impl IntoResponse {
        let _user = match get_user_from_session(auth_session, config.clone()).await {
            Ok(x) => x,
            Err(response) => {
                return response.into_response();
            }
        };

        let project = match get_project(config.pg_pool.clone(), project_id).await {
            Ok(Some(x)) => x,
            Ok(None) => {
                info!("Project {project_id} was requested but does not exist.");
                return (StatusCode::NOT_FOUND).into_response();
            }
            Err(e) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal server error because I cannot get project {project_id} by id: {e}. {error_uuid}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                )
                    .into_response();
            }
        };
        project.display_header_only().into_response()
    }

    /// Get an individual project by ID, show its users.
    pub(super) async fn project_with_users(
        auth_session: AuthSession,
        Extension(config): Extension<Arc<Config>>,
        Path(project_id): Path<i32>,
    ) -> impl IntoResponse {
        let user = match get_user_from_session(auth_session, config.clone()).await {
            Ok(x) => x,
            Err(response) => {
                return response.into_response();
            }
        };

        let project = match get_project(config.pg_pool.clone(), project_id).await {
            Ok(Some(x)) => x,
            Ok(None) => {
                info!("Project {project_id} was requested but does not exist.");
                return (StatusCode::NOT_FOUND).into_response();
            }
            Err(e) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal server error because I cannot get project {project_id} by id: {e}. {error_uuid}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                )
                    .into_response();
            }
        };
        // find out whether the user is an admin of this group
        let permission = match user.global_permission {
            UserPermission::Admin => UserPermission::Admin,
            UserPermission::User => match project.local_permission_for_user(&user) {
                Some(x) => x,
                None => UserPermission::User,
            },
        };
        // template it with header_only
        project.display_with_users(permission).into_response()
    }

    #[derive(askama_axum::Template)]
    #[template(path = "project/new_member.html")]
    pub(super) struct ProjectNewMemberTemplate {
        project_id: i32,
    }
    pub(super) async fn project_new_member_template(
        auth_session: AuthSession,
        Extension(config): Extension<Arc<Config>>,
        Path(project_id): Path<i32>,
    ) -> impl IntoResponse {
        ProjectNewMemberTemplate { project_id }.into_response()
    }
}

pub(super) mod post {
    use std::sync::Arc;

    use askama_axum::IntoResponse;
    use axum::{extract::Path, http::StatusCode, Extension, Form};
    use serde::Deserialize;
    use tracing::{error, info, trace, warn, Level};
    use uuid::Uuid;

    use crate::{
        config::Config,
        db::{add_project, get_person, get_persons_with_similar_name, get_project, update_member_permission, update_project_members},
        types::{HasID, NoID, Project, UserPermission},
        web_server::{actions::{add_member_to_project, AddMemberError}, login::AuthSession, protected::get_user_from_session, InternalServerErrorTemplate},
    };

    #[derive(Deserialize, Debug)]
    pub(super) struct NewProjectData {
        name: String,
    }

    #[tracing::instrument(level=Level::TRACE, skip(auth_session, config))]
    pub(super) async fn project_new(
        auth_session: AuthSession,
        Extension(config): Extension<Arc<Config>>,
        Form(new_form): Form<NewProjectData>,
    ) -> impl IntoResponse {
        let user = if let Some(x) = auth_session.user {
            x
        } else {
            let error_uuid = Uuid::new_v4();
            warn!("Sending internal server error because there is no user in the auth session. uuid: {error_uuid}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                InternalServerErrorTemplate { error_uuid },
            )
                .into_response();
        };

        let user_obj = match get_person(config.pg_pool.clone(), &user.username).await {
            Ok(Some(x)) => x,
            Ok(None) => {
                let error_uuid = Uuid::new_v4();
                // this should fix itself on the next LDAP->DB sync period
                warn!("Sending internal server error because a logged-in user did not exist. {error_uuid}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                )
                    .into_response();
            }
            Err(e) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal Server error because I cannot get a user by name: {e}. {error_uuid}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                )
                    .into_response();
            }
        };

        if user_obj.global_permission == UserPermission::User {
            warn!(
                "User {} tried to create a project, but is not a global admin.",
                user.username
            );
            return StatusCode::UNAUTHORIZED.into_response();
        };

        // create the new project
        let project = Project::<NoID>::new((), new_form.name);
        match add_project(config.pg_pool.clone(), project).await {
            Ok(x) => {
                // only global admins can create projects, so we template it with admin privileges
                x.display_with_users(UserPermission::Admin).into_response()
            }
            Err(e) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal Server error because I cannot insert a new project: {e}. {error_uuid}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                )
                    .into_response();
            }
        }
    }

    #[derive(Deserialize)]
    pub(super) struct ProjectNewMemberFormData {
        username: String,
    }
    /// Add an existing user as member to an existing project
    pub(super) async fn project_new_member(
        auth_session: AuthSession,
        Extension(config): Extension<Arc<Config>>,
        Path(project_id): Path<i32>,
        Form(form): Form<ProjectNewMemberFormData>,
    ) -> impl IntoResponse {
        // get the user this name belongs to
        // get the project from ID
        // add that uer to the given project
        let requester = match get_user_from_session(auth_session, config.clone()).await {
            Ok(x) => x,
            Err(e) => {
                return e.into_response();
            },
        };

        match add_member_to_project(config.clone(), &requester, &form.username, project_id).await {
            Ok((new_member, project)) => {
                let requester_is_now_admin = match project.local_permission_for_user(&requester) {
                    Some(UserPermission::Admin) => { true }
                    Some(UserPermission::User) => {
                        requester.is_global_admin()
                    }
                    None => {
                        requester.is_global_admin()
                    }
                };
                new_member.display(project.project_id(), UserPermission::new_from_is_admin(requester_is_now_admin), new_member.global_permission).into_response()
            }
            Err(AddMemberError::ProjectDoesNotExist) => {
                warn!("Sending 404 because no project with id {project_id} exists.");
                StatusCode::NOT_FOUND.into_response()
            }
            Err(AddMemberError::PersonDoesNotExist) => {
                warn!("Sending 400 because the person {} does not exist.", form.username);
                StatusCode::BAD_REQUEST.into_response()
            }
            Err(AddMemberError::RequesterHasNoPermission(project_name)) => {
                warn!("Sending 401 because user {} is not authorized to add member to group {}.", requester.name, project_name);
                StatusCode::UNAUTHORIZED.into_response()
            }
            Err(AddMemberError::DB(e)) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal server error because a DB interaction failed: {e}. {error_uuid}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                ).into_response()
            }
        }
    }

    #[derive(Deserialize, Debug)]
    pub(super) struct UserSearchFormData {
        username: String,
    }
    #[derive(askama_axum::Template)]
    #[template(path = "search/user_results.html")]
    pub(super) struct UserSearchResultsTemplate {
        results: Vec<(String, String)>,
    }

    #[tracing::instrument(level=Level::TRACE, skip(auth_session, config))]
    pub(super) async fn search_user_results(
        auth_session: AuthSession,
        Extension(config): Extension<Arc<Config>>,
        Form(form): Form<UserSearchFormData>,
    ) -> impl IntoResponse {
        // get users whith name similar to the form.username
        let persons = match get_persons_with_similar_name(config.pg_pool.clone(), &form.username).await {
            Ok(x) => x,
            Err(e) => {
            let error_uuid = Uuid::new_v4();
            warn!("Sending internal server error because I cannot get persons with similar name: {e}. {error_uuid}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                InternalServerErrorTemplate { error_uuid },
            )
                .into_response();
                }
        };
        let results = persons.into_iter().map(|p| (format!("{} {} ({})", p.firstname.unwrap_or("".to_owned()), p.surname.unwrap_or("".to_owned()), p.name), p.name)).collect();
        UserSearchResultsTemplate { results, }.into_response()
    }

    #[derive(Deserialize, Debug)]
    pub(crate) struct SetMemberPermissionForm {
        username: String,
        is_local_admin: bool,
    }
    pub(super) async fn project_set_member_permission(
        auth_session: AuthSession,
        Extension(config): Extension<Arc<Config>>,
        Path(project_id): Path<i32>,
        Form(form): Form<SetMemberPermissionForm>,
        ) -> impl IntoResponse {
        // get the user this name belongs to
        // get the project from ID
        let user = match get_user_from_session(auth_session, config.clone()).await {
            Ok(x) => x,
            Err(e) => {
                return e.into_response();
            },
        };

        // the permission to do this depends on the project, so we need to get that before checking
        // permission
        let project = match get_project(config.pg_pool.clone(), project_id).await {
            Ok(Some(x)) => x,
            Ok(None) => {
                warn!("Sending 404 because no project with id {project_id} exists.");
                return StatusCode::NOT_FOUND.into_response();
            }
            Err(e) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal server error because I cannot get project by id: {e}. {error_uuid}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                ).into_response();
            }
        };

        let user_may_set_member_permissions = match project.local_permission_for_user(&user) {
            Some(UserPermission::Admin) => { true }
            Some(UserPermission::User) => {
                user.is_global_admin()
            }
            None => {
                user.is_global_admin()
            }
        };
        if !user_may_set_member_permissions {
                warn!("Sending 401 because user {} is not authorized to set member permissions on group {}.", user.name, project.name);
                return StatusCode::UNAUTHORIZED.into_response();
        };

        // The user is allowed to set member permissions on this project.
        // Now we need to make sure the requested member is actually a known user.
        let change_member = match get_person(config.pg_pool.clone(), &form.username).await {
            Ok(Some(x)) => x,
            Ok(None) => {
                warn!("Sending 400 because the person {} does not exist.", form.username);
                return StatusCode::BAD_REQUEST.into_response();
            }
            Err(e) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal server error because I cannot get the new member by name: {e}. {error_uuid}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                ).into_response();
            }
        };

        let new_perm = UserPermission::new_from_is_admin(form.is_local_admin);
        match update_member_permission(config.pg_pool.clone(), project_id, change_member.person_id(), new_perm).await {
            Ok(()) => {
                info!("Updated permission for {} in {}; is now {}; request made by {}.", change_member.name, project.name, new_perm, user.name);
                change_member.display(project_id, UserPermission::new_from_is_admin(user_may_set_member_permissions), new_perm).into_response()
            }
            Err(e) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal server error because I cannot remove {} from {}: {e}. {error_uuid}", change_member.name, project.name);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                ).into_response()
            }
        }
    }
}

pub(super) mod delete {
    use std::sync::Arc;

    use askama_axum::IntoResponse;
    use axum::{extract::{Path, Query}, http::StatusCode, Extension, Form};
    use serde::Deserialize;
    use tracing::{info, warn};
    use uuid::Uuid;

    use crate::{config::Config, db::{get_person, get_project, remove_members, update_project_members}, types::UserPermission, web_server::{actions::{remove_member_from_project, RemoveMemberError}, login::AuthSession, InternalServerErrorTemplate}};

    use super::get_user_from_session;

    #[derive(Deserialize, Debug)]
    pub(crate) struct RemoveMemberForm {
        username: String,
    }
    pub(super) async fn project_remove_member(
        auth_session: AuthSession,
        Extension(config): Extension<Arc<Config>>,
        Path(project_id): Path<i32>,
        Query(form): Query<RemoveMemberForm>,
        ) -> impl IntoResponse {
        // get the user this name belongs to
        let requester = match get_user_from_session(auth_session, config.clone()).await {
            Ok(x) => x,
            Err(e) => {
                return e.into_response();
            },
        };

        match remove_member_from_project(config.clone(), &requester, &form.username, project_id).await {
            Ok((new_member, project)) => {
                let requester_is_now_admin = match project.local_permission_for_user(&requester) {
                    Some(UserPermission::Admin) => { true }
                    Some(UserPermission::User) => {
                        requester.is_global_admin()
                    }
                    None => {
                        requester.is_global_admin()
                    }
                };
                (StatusCode::OK, "").into_response()
            }
            Err(RemoveMemberError::ProjectDoesNotExist) => {
                warn!("Sending 404 because no project with id {project_id} exists.");
                StatusCode::NOT_FOUND.into_response()
            }
            Err(RemoveMemberError::PersonDoesNotExist) => {
                warn!("Sending 400 because the person {} does not exist.", form.username);
                StatusCode::BAD_REQUEST.into_response()
            }
            Err(RemoveMemberError::RequesterHasNoPermission(project_name)) => {
                warn!("Sending 401 because user {} is not authorized to add member to group {}.", requester.name, project_name);
                StatusCode::UNAUTHORIZED.into_response()
            }
            Err(RemoveMemberError::DB(e)) => {
                let error_uuid = Uuid::new_v4();
                warn!("Sending internal server error because a DB interaction failed: {e}. {error_uuid}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    InternalServerErrorTemplate { error_uuid },
                ).into_response()
            }
        }
    }
}
