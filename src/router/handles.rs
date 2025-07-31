use axum::http::{StatusCode, HeaderMap};

use crate::net::body;
use crate::net::Error;
use crate::sec::authn::Initiator;
use crate::state;

pub mod admin;
pub mod api;
pub mod journals;
pub mod login;
pub mod logout;
pub mod peers;
pub mod register;
pub mod settings;
pub mod verify;

pub async fn ping() -> (StatusCode, &'static str) {
    (StatusCode::OK, "pong")
}

pub async fn retrieve_root(
    state: state::SharedState,
    _: Initiator,
    headers: HeaderMap,
) -> Result<body::Json<serde_json::Value>, Error> {
    body::assert_html(state.templates(), &headers)?;

    Ok(body::Json(serde_json::json!({
        "message": "okay"
    })))
}
