use axum::http::HeaderMap;
use axum::response::IntoResponse;

use crate::db::ids::UserId;
use crate::net::{body, header, Error};
use crate::sec::authn::Initiator;

#[derive(Debug, serde::Serialize)]
pub struct MyInfo {
    id: UserId,
    username: String,
}

pub async fn retrieve_me(
    initiator: Initiator,
    headers: HeaderMap,
) -> Result<impl IntoResponse, Error> {
    if header::is_accepting_html(&headers)? {
        Ok(header::Location::to("/").into_response())
    } else {
        Ok(body::Json(MyInfo {
            id: initiator.user.id,
            username: initiator.user.username,
        })
        .into_response())
    }
}
