use axum::Router;
use axum::http::StatusCode;
use axum::routing::{get, post};

use crate::state::SharedState;
use crate::sec::authn::ApiInitiator;

mod authn;
mod journals;

pub fn build(_state: &SharedState) -> Router<SharedState> {
    Router::new()
        .route("/ping", get(check))
        .route("/authn", post(authn::post)
            .patch(authn::patch))
        .route("/journals", post(journals::post))
        .route("/journals/:journals_id/entries", post(journals::journals_id::entries::post))
}

async fn check(_: ApiInitiator) -> StatusCode {
    StatusCode::OK
}
