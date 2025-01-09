use axum::Router;
use axum::http::{Uri, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::state;
use crate::error;
use crate::router::{body, macros};

mod users;
mod groups;
mod roles;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/", get(retrieve_admin))
        .route("/users", get(users::retrieve_users)
            .post(users::create_user))
        .route("/users/new", get(users::retrieve_user))
        .route("/users/:users_id", get(users::retrieve_user)
            .patch(users::update_user)
            .delete(users::delete_user))
        .route("/groups", get(groups::retrieve_groups)
            .post(groups::create_group))
        .route("/groups/new", get(groups::retrieve_group))
        .route("/groups/:groups_id", get(groups::retrieve_group)
            .patch(groups::update_group)
            .delete(groups::delete_group))
        .route("/roles", get(roles::retrieve_roles)
            .post(roles::create_role))
        .route("/roles/new", get(roles::retrieve_role))
        .route("/roles/:role_id", get(roles::retrieve_role)
            .patch(roles::update_role)
            .delete(roles::delete_role))
}

async fn retrieve_admin(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap
) -> Result<Response, error::Error> {
    let conn = state.db_conn().await?;

    let _initiator = macros::require_initiator!(
        &conn,
        &headers,
        Some(uri.clone())
    );

    macros::res_if_html!(state.templates(), &headers);

    Ok(body::Json("okay").into_response())
}