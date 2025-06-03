use std::fmt::Display;

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::error::{log_prefix_error, BoxDynError};
use crate::net::body::serialize_json;

pub struct Empty;

pub enum Error<T = Empty> {
    Inner(T),
    Defined {
        response: Response,
        msg: Option<String>,
        src: Option<BoxDynError>,
    },
}

impl<T> IntoResponse for Error<T>
where
    T: IntoResponse + Display,
{
    fn into_response(self) -> Response {
        match self {
            Self::Inner(value) => {
                tracing::error!("{value}");

                value.into_response()
            }
            Self::Defined { response, msg, src } => {
                match (msg, src) {
                    (Some(msg), Some(src)) => {
                        log_prefix_error(&format!("error during request: {msg}"), &*src)
                    }
                    (Some(msg), None) => tracing::error!("error during request: {msg}"),
                    (None, Some(src)) => log_prefix_error("error during request", &*src),
                    (None, None) => tracing::error!("error during request"),
                }

                response
            }
        }
    }
}

impl Display for Empty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Empty error response")
    }
}

impl IntoResponse for Empty {
    fn into_response(self) -> Response {
        json_server_error()
    }
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

macro_rules! simple_from {
    ($e:path) => {
        impl<T> From<$e> for crate::net::error::Error<T> {
            fn from(err: $e) -> Self {
                Self::Defined {
                    response: crate::net::error::json_server_error(),
                    msg: None,
                    src: Some(err.into()),
                }
            }
        }
    };
    ($e:path, $m:expr) => {
        impl<T> From<$e> for crate::net::error::Error<T> {
            fn from(err: $e) -> Self {
                Self::Defined {
                    response: crate::net::error::json_server_error(),
                    msg: Some($m.into()),
                    src: Some(err.into()),
                }
            }
        }
    };
}

pub(crate) use simple_from;

simple_from!(crate::db::PgError);
simple_from!(crate::db::PoolError);
simple_from!(crate::error::Error);
simple_from!(crate::sec::otp::UnixTimestampError);
