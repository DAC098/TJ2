use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::error;
use crate::router::{body, macros};
use crate::sec::authn::Initiator;
use crate::state;

mod auth;
mod peer_client;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/", get(get_))
        .route("/auth", get(auth::get).patch(auth::patch))
        .route(
            "/peer_client",
            get(peer_client::get)
                .post(peer_client::post)
                .delete(peer_client::delete),
        )
}

async fn get_(
    state: state::SharedState,
    _initiator: Initiator,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    Ok(body::Json("ok").into_response())
}
