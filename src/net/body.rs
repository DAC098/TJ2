use async_trait::async_trait;
use axum::body::Body;
use axum::extract::{FromRequest, Request};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::{BufMut, Bytes, BytesMut};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::log_prefix_error;
use crate::error::{self, Context};
use crate::net::header::is_accepting_html;
use crate::net::Error as NetError;

pub fn json_bytes(data: &impl Serialize) -> Result<Bytes, serde_json::Error> {
    let mut buf = BytesMut::with_capacity(128).writer();
    serde_json::to_writer(&mut buf, data)?;

    Ok(buf.into_inner().freeze())
}

pub fn serialize_json(
    status: StatusCode,
    data: &impl Serialize,
) -> Result<Response, serde_json::Error> {
    json_bytes(data).map(|buf| {
        Response::builder()
            .status(status)
            .header("content-type", "application/json; charset=utf-8")
            .header("content-length", buf.len())
            .body(Body::from(buf))
            .unwrap()
    })
}

/// creates an error response with the given message
pub fn json_error_response<K, M>(status: StatusCode, kind: K, msg: M) -> Response<Body>
where
    K: Into<String>,
    M: Into<String>,
{
    let json = serde_json::json!({
        "error": kind.into(),
        "message": msg.into(),
    });

    serialize_json(status, &json).expect("failed to parse error json")
}

pub fn json_server_error() -> Response<Body> {
    json_error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "ServerError",
        "internal server error when handling request",
    )
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

                json_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "JsonError",
                    "failed to serialize json",
                )
            }
        }
    }
}

#[async_trait]
impl<S, T> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::from_request(req, state).await {
            Ok(axum::Json(inner)) => Ok(Self(inner)),
            Err(err) => {
                log_prefix_error("failed to parse json request body", &err);

                Err(json_error_response(
                    StatusCode::BAD_REQUEST,
                    "InvalidJson",
                    "the provided json is not valid",
                ))
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

pub fn error_html<T>(message: Option<T>) -> Html<String>
where
    T: Into<String>,
{
    let message = message.map(|value| value.into()).unwrap_or(String::from(
        "There was a problem when requesting this page",
    ));

    Html::new(format!(
        "
<!DOCTYPE html>\
<html lang=\"en\">\
<head>\
    <title>TJ2 - Error</title>\
</head>\
<body>\
    <main>{message}</main>\
</body>\
</html>"
    ))
}

pub fn assert_html<E>(templates: &tera::Tera, headers: &HeaderMap) -> Result<(), NetError<E>> {
    if is_accepting_html(headers)? {
        let context = tera::Context::new();

        match templates.render("pages/spa", &context) {
            Ok(rendered) => Err(Html::new(rendered).into_response().into()),
            Err(err) => Err(NetError::Defined {
                response: error_html(None::<&str>).into_response(),
                msg: None,
                src: Some(err.into()),
            }),
        }
    } else {
        Ok(())
    }
}
