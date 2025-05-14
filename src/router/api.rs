use axum::Router;
use axum::http::StatusCode;
use axum::routing::{get, post};

use crate::state::SharedState;
use crate::sec::authn::ApiInitiator;

mod authn;

pub fn build(_state: &SharedState) -> Router<SharedState> {
    Router::new()
        .route("/ping", get(check))
        .route("/authn", post(authn::post)
            .patch(authn::patch))
}

async fn check(initiator: ApiInitiator) -> StatusCode {
    StatusCode::OK
}
