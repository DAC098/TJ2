use std::fmt::Display;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::error::{log_prefix_error, BoxDynError};
use crate::net::body::{json_error_response, json_server_error};

pub struct Empty;

pub enum Error<T = Empty> {
    Inner(T),
    Defined {
        response: Response,
        msg: Option<String>,
        src: Option<BoxDynError>,
    },
}

impl<T> Error<T> {
    pub fn general() -> Self {
        Self::Defined {
            response: json_server_error(),
            msg: None,
            src: None,
        }
    }

    pub fn source<E>(src: E) -> Self
    where
        E: Into<BoxDynError>,
    {
        Self::Defined {
            response: json_server_error(),
            msg: None,
            src: Some(src.into()),
        }
    }

    pub fn message<M>(msg: M) -> Self
    where
        M: Into<String>
    {
        Self::Defined {
            response: json_server_error(),
            msg: Some(msg.into()),
            src: None
        }
    }

    pub fn with_source<E>(self, src: E) -> Self
    where
        E: Into<BoxDynError>,
    {
        match self {
            Self::Defined { response, msg, .. } => Self::Defined {
                response,
                msg,
                src: Some(src.into()),
            },
            _ => self,
        }
    }
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
                    (None, None) => {}
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

macro_rules! simple_from {
    ($e:path) => {
        impl<T> From<$e> for crate::net::error::Error<T> {
            fn from(err: $e) -> Self {
                Self::Defined {
                    response: crate::net::body::json_server_error(),
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
                    response: crate::net::body::json_server_error(),
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
simple_from!(crate::sec::password::HashError);

simple_from!(axum::Error);
simple_from!(axum::http::Error);
simple_from!(std::io::Error);
simple_from!(tj2_lib::sec::pki::PrivateKeyError);

impl<T> From<axum::http::header::ToStrError> for Error<T> {
    fn from(err: axum::http::header::ToStrError) -> Self {
        Self::Defined {
            response: json_error_response(
                StatusCode::BAD_REQUEST,
                "InvalidHeader",
                "a provided header value is not valid utf-8",
            ),
            msg: None,
            src: Some(err.into()),
        }
    }
}

impl<T> From<crate::sec::authz::PermissionError> for Error<T> {
    fn from(err: crate::sec::authz::PermissionError) -> Self {
        match err {
            crate::sec::authz::PermissionError::Denied => Self::Defined {
                response: json_error_response(
                    StatusCode::FORBIDDEN,
                    "PermissionDenied",
                    "you do not have permission to access this resource",
                ),
                msg: None,
                src: None,
            },
            crate::sec::authz::PermissionError::Db(err) => err.into(),
        }
    }
}

use crate::sec::authn::InitiatorError;

impl<T> From<InitiatorError> for Error<T> {
    fn from(err: InitiatorError) -> Self {
        match err {
            InitiatorError::Token(_)
            | InitiatorError::SessionIdNotFound
            | InitiatorError::SessionNotFound
            | InitiatorError::UserNotFound(_)
            | InitiatorError::Unauthenticated(_)
            | InitiatorError::Unverified(_)
            | InitiatorError::SessionExpired(_) => Self::Defined {
                response: json_error_response(
                    StatusCode::UNAUTHORIZED,
                    "InvalidSession",
                    "you current session is invalid",
                ),
                msg: None,
                src: Some(err.into()),
            },
            InitiatorError::HeaderStr(err) => err.into(),
            InitiatorError::DbPg(err) => err.into(),
        }
    }
}

use crate::sec::authn::ApiInitiatorError;

impl<T> From<ApiInitiatorError> for Error<T> {
    fn from(err: ApiInitiatorError) -> Self {
        match err {
            ApiInitiatorError::InvalidAuthorization
            | ApiInitiatorError::NotFound
            | ApiInitiatorError::UserNotFound(_)
            | ApiInitiatorError::Unauthenticated(_)
            | ApiInitiatorError::Expired(_) => Self::Defined {
                response: json_error_response(
                    StatusCode::UNAUTHORIZED,
                    "InvalidApiToken",
                    "your current api token is invalid",
                ),
                msg: None,
                src: Some(err.into()),
            },
            ApiInitiatorError::DbPg(err) => err.into(),
        }
    }
}

use crate::fs::RemovedFileError;

impl<T> From<RemovedFileError> for Error<T> {
    fn from(err: RemovedFileError) -> Self {
        match err {
            RemovedFileError::Io(err) => Self::from(err),
            _ => Self::source(err),
        }
    }
}

use crate::fs::FileCreaterError;

impl<T> From<FileCreaterError> for Error<T> {
    fn from(err: FileCreaterError) -> Self {
        match err {
            FileCreaterError::Io(err) => Self::from(err),
            _ => Self::source(err),
        }
    }
}

impl<T> From<Response> for Error<T> {
    fn from(response: Response) -> Self {
        Self::Defined {
            response,
            msg: None,
            src: None,
        }
    }
}
