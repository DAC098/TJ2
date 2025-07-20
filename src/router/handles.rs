use axum::http::HeaderMap;

use crate::net::body;
use crate::net::Error;
use crate::sec::authn::Initiator;
use crate::state;

pub async fn send_html(
    state: state::SharedState,
    _: Initiator,
    headers: HeaderMap,
) -> Result<body::Json<&'static str>, Error> {
    body::assert_html(state.templates(), &headers)?;

    Ok(body::Json("okay"))
}

pub async fn send_json(_: Initiator) -> Result<body::Json<&'static str>, Error> {
    Ok(body::Json("okay"))
}
