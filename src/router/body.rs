use async_trait::async_trait;
use axum::body::Body;
use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bytes::{BufMut, BytesMut};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::log_prefix_error;
use crate::error::{self, Context};
use crate::state;

fn serialize_json(
    status: StatusCode,
    data: &impl Serialize,
) -> Result<Response, serde_json::Error> {
    let froze = {
        let mut buf = BytesMut::with_capacity(128).writer();
        serde_json::to_writer(&mut buf, data)?;

        buf.into_inner().freeze()
    };

    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json; charset=utf-8")
        .header("content-length", froze.len())
        .body(Body::from(froze))
        .unwrap())
}

fn error_json(status: StatusCode, error: &str, message: Option<&str>) -> Response {
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
    T: Serialize,
{
    fn into_response(self) -> Response {
        match serialize_json(StatusCode::OK, &self.0) {
            Ok(res) => res,
            Err(err) => {
                log_prefix_error("failed to serialize struct to json response", &err);

                error_json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_SERVER_ERROR",
                    None,
                )
            }
        }
    }
}

#[async_trait]
impl<T> FromRequest<state::SharedState> for Json<T>
where
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(
        req: Request,
        state: &state::SharedState,
    ) -> Result<Self, Self::Rejection> {
        match axum::Json::from_request(req, state).await {
            Ok(axum::Json(inner)) => Ok(Self(inner)),
            Err(err) => {
                log_prefix_error("failed to parse json request body", &err);

                Err(error_json(StatusCode::BAD_REQUEST, "INVALID_JSON", None))
            }
        }
    }
}

#[async_trait]
impl<T> FromRequest<()> for Json<T>
where
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &()) -> Result<Self, Self::Rejection> {
        match axum::Json::from_request(req, state).await {
            Ok(axum::Json(inner)) => Ok(Self(inner)),
            Err(err) => {
                log_prefix_error("failed to parse json request body", &err);

                Err(error_json(StatusCode::BAD_REQUEST, "INVALID_JSON", None))
            }
        }
    }
}

pub struct Html<T = String> {
    body: T,
}

impl<T> Html<T> {
    pub fn new(body: T) -> Self {
        Self { body }
    }
}

impl IntoResponse for Html<String> {
    fn into_response(self) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/html; charset=utf-8")
            .header("content-length", self.body.len())
            .body(self.body.into())
            .unwrap()
    }
}

impl IntoResponse for Html<&str> {
    fn into_response(self) -> Response<Body> {
        let owned = self.body.to_owned();

        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/html; charset=utf-8")
            .header("content-length", owned.len())
            .body(owned.into())
            .unwrap()
    }
}

pub struct SpaPage(Html<String>);

impl SpaPage {
    pub fn new(templates: &tera::Tera) -> Result<Self, error::Error> {
        let context = tera::Context::new();
        let page_index = templates
            .render("pages/spa", &context)
            .context("failed to render index page")?;

        Ok(Self(Html::new(page_index)))
    }
}

impl IntoResponse for SpaPage {
    fn into_response(self) -> Response<Body> {
        self.0.into_response()
    }
}
