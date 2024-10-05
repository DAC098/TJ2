use async_trait::async_trait;
use axum::body::Body;
use axum::extract::{Request, FromRequest};
use axum::http::StatusCode;
use axum::response::{Response, IntoResponse};
use bytes::{BytesMut, BufMut};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::log_prefix_error;
use crate::state;

fn serialize_json(
    status: StatusCode,
    data: &impl Serialize
) -> Result<Response, serde_json::Error> {
    let froze = {
        let mut buf = BytesMut::with_capacity(128).writer();
        serde_json::to_writer(&mut buf, data)?;

        buf.into_inner().freeze()
    };

    Ok(Response::builder()
       .status(status)
       .header("content-type", "application/json")
       .header("content-length", froze.len())
       .body(Body::from(froze))
       .unwrap())
}

fn error_json(
    status: StatusCode,
    error: &str,
    message: Option<&str>
) -> Response {
    let body = if let Some(message) = message {
        format!(r#"{{"error": "{error}", "message": "{message}"}}"#)
    } else {
        format!(r#"{{"error": "{error}"}}"#)
    };

    Response::builder()
       .status(status)
       .header("content-type", "application/json")
       .header("content-length", body.len())
       .body(Body::from(body))
       .unwrap()
}

pub struct Json<T>(pub T);

impl<T> IntoResponse for Json<T>
where
    T: Serialize
{
    fn into_response(self) -> Response {
        match serialize_json(StatusCode::OK, &self.0) {
            Ok(res) => res,
            Err(err) => {
                log_prefix_error(
                    "failed to serialize struct to json response",
                    &err
                );

                error_json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_SERVER_ERROR",
                    None
                )
            }
        }
    }
}

#[async_trait]
impl<T> FromRequest<state::SharedState> for Json<T>
where
    T: DeserializeOwned
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &state::SharedState) -> Result<Self, Self::Rejection> {
        match axum::Json::from_request(req, state).await {
            Ok(axum::Json(inner)) => Ok(Self(inner)),
            Err(err) => {
                log_prefix_error(
                    "failed to parse json request body",
                    &err
                );

                Err(error_json(
                    StatusCode::BAD_REQUEST,
                    "INVALID_JSON",
                    None
                ))
            }
        }
    }
}
