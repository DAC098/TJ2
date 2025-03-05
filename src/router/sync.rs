use axum::Router;
use axum::extract::Path;
use axum::http::{StatusCode, Uri, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use chrono::{Utc, DateTime};
use futures::StreamExt;
use serde::{Serialize, Deserialize};

use crate::error;
use crate::router::body;
use crate::state;
use crate::sync;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/entries", post(receive_entry))
}

async fn receive_entry(
    state: state::SharedState,
    body::Json(json): body::Json<sync::journal::EntrySync>,
) -> Result<StatusCode, error::Error> {
    tracing::debug!("received entry from server: {}", json.uid);

    Ok(StatusCode::CREATED)
}
