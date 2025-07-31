use axum::routing::get;
use axum::Router;

use crate::net::response::send_html;
use crate::state;

mod groups;
mod invites;
mod roles;
mod users;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/", get(send_html))
        .route("/users", get(users::search_users).post(users::create_user))
        .route("/users/new", get(send_html))
        .route(
            "/users/:users_id",
            get(users::retrieve_user)
                .patch(users::update_user)
                .delete(users::delete_user),
        )
        .route(
            "/groups",
            get(groups::retrieve_groups).post(groups::create_group),
        )
        .route("/groups/new", get(send_html))
        .route(
            "/groups/:groups_id",
            get(groups::retrieve_group)
                .patch(groups::update_group)
                .delete(groups::delete_group),
        )
        .route(
            "/roles",
            get(roles::retrieve_roles).post(roles::create_role),
        )
        .route("/roles/new", get(send_html))
        .route(
            "/roles/:role_id",
            get(roles::retrieve_role)
                .patch(roles::update_role)
                .delete(roles::delete_role),
        )
        .route(
            "/invites",
            get(invites::search_invites)
                .post(invites::create_invite)
                .delete(invites::delete_invite),
        )
}
