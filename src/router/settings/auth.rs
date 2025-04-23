use axum::http::HeaderMap;
use axum::response::{Response, IntoResponse};

use crate::error;
use crate::router::{body, macros};
use crate::state;
use crate::sec::authn::Initiator;

pub async fn get(
    state: state::SharedState,
    _initiator: Initiator,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    Ok(body::Json("ok").into_response())
}

pub async fn post(
    _state: state::SharedState,
    _initiator: Initiator,
) -> Result<(), error::Error> {
    Ok(())
}
