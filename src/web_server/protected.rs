use std::sync::Arc;

/// The routes protected by a login
use axum::{routing::{get, post}, Extension, Router};

fn error_display(s: &str) -> String {
    // we cannot control hx-swap separately for hx-target and hx-target-error
    // so we swap outer-html and add the surrounding div all the time
    format!("<div class=\"text-red-500 flex justify-center\" id=\"error_display\" _=\"on htmx:beforeSend from elsewhere set my innerHTML to ''\">{}</div>", s)
}

pub(crate) fn create_protected_router() -> Router {
    Router::new()
        .route("/", get(self::get::root))
        .route("/web/project/new", get(self::get::project_new_template))
        .route("/web/project/new", post(self::post::project_new))
}

pub(super) mod get {
    use crate::{
        db::{get_person, get_projects}, types::{HasID, Project, UserPermission}, web_server::{login::AuthSession, InternalServerErrorTemplate}
    };

    use super::*;
    use core::borrow::Borrow;

    use askama::Template;
    use askama_axum::IntoResponse;
    use axum::http::StatusCode;
    use tracing::{info, warn};
    use uuid::Uuid;

    use crate::config::Config;

    #[derive(Template)]
    #[template(path = "landing/complete.html", escape="none")]
    struct LandingAsUser {
        username: String,
        projects: Vec<Project<HasID>>,
        permission: UserPermission,
    }

    #[derive(Template)]
    #[template(path = "landing/not_yet_synced.html")]
    struct LandingNotYetSynced{
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
        info!("Projects: {projects:?}");
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
            Some(person) => {
                LandingAsUser {
                    username: person.name,
                    projects,
                    permission: person.global_permission,
                }
                .into_response()
            }
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


    #[derive(Template, Debug)]
    #[template(path = "project/new.html")]
    struct NewProject {
    }

    /// return the html form for adding a new project
    pub(super) async fn project_new_template(
    ) -> impl IntoResponse {
        NewProject {}
         .into_response()
    }

    pub(super) async fn project_header_only(
        auth_session: AuthSession,
        Extension(config): Extension<Arc<Config>>,) -> impl IntoResponse {
        todo!()
    }
}

pub(super) mod post {
    use std::sync::Arc;

    use askama::Template;
    use askama_axum::IntoResponse;
    use axum::{http::StatusCode, Extension, Form};
    use serde::Deserialize;
    use tracing::{warn, Level};
    use uuid::Uuid;

    use crate::{config::Config, db::{add_project, get_person}, types::{HasID, NoID, Project, UserPermission}, web_server::{login::AuthSession, InternalServerErrorTemplate}};

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
            },
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
            warn!("User {} tried to create a project, but is not a global admin.", user.username);
            return StatusCode::UNAUTHORIZED.into_response();
        };

        // create the new project
        let project = Project::<NoID>::new((), new_form.name);
        match add_project(config.pg_pool.clone(), project).await {
            Ok(x) => {
                x.display_with_users().into_response()
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
}
