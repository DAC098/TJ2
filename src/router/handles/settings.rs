use axum::routing::get;
use axum::Router;

use crate::net::response::send_html;
use crate::state;

mod auth;
mod peer_client;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/", get(send_html))
        .route("/auth", get(auth::get).patch(auth::patch))
        .route(
            "/peer_client",
            get(peer_client::get)
                .post(peer_client::post)
                .delete(peer_client::delete),
        )
}
