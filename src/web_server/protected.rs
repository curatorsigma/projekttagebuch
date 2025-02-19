use std::sync::Arc;

/// The routes protected by a login
use axum::{routing::get, Extension, Router};

fn error_display(s: &str) -> String {
    // we cannot control hx-swap separately for hx-target and hx-target-error
    // so we swap outer-html and add the surrounding div all the time
    format!("<div class=\"text-red-500 flex justify-center\" id=\"error_display\" _=\"on htmx:beforeSend from elsewhere set my innerHTML to ''\">{}</div>", s)
}

pub(crate) fn create_protected_router() -> Router {
    Router::new().route("/", get(self::get::root))
}

pub(super) mod get {
    use crate::{
        types::{HasID, Project, UserPermission},
        web_server::{login::AuthSession, InternalServerErrorTemplate},
    };

    use super::*;

    use askama::Template;
    use askama_axum::IntoResponse;
    use axum::http::StatusCode;
    use tracing::warn;
    use uuid::Uuid;

    use crate::config::Config;

    #[derive(Template)]
    #[template(path = "landing/complete.html")]
    struct LandingAsUser {
        username: String,
        projects: Vec<Project<HasID>>,
        permission: UserPermission,
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

        LandingAsUser {
            username: user.username,
            projects: vec![],
            permission: UserPermission::User,
        }
        .into_response()
    }
}
