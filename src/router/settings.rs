use axum::Router;
use axum::http::HeaderMap;
use axum::response::{Response, IntoResponse};
use axum::routing::get;

use crate::error;
use crate::router::{body, macros};
use crate::state;
use crate::sec::authn::Initiator;

mod auth;
mod peer_client;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/", get(get_))
        .route("/auth", get(auth::get)
            .patch(auth::patch))
}

async fn get_(
    state: state::SharedState,
    _initiator: Initiator,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    Ok(body::Json("ok").into_response())
}
