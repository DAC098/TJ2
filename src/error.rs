use std::convert::Infallible;
use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub type BoxDynError = Box<dyn std::error::Error + Send + Sync>;

/// creates an error response with the given message
pub fn error_response<S>(msg: S) -> Response<Body>
where
    S: Into<String>,
{
    let message = msg.into();

    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("content-type", "text/plain; charset=utf-8")
        .header("content-length", message.len())
        .body(Body::from(message))
        .unwrap()
}

/// the common error struct for use in the server
#[derive(Debug, thiserror::Error)]
pub struct Error {
    cxt: String,

    #[source]
    src: Option<BoxDynError>,
}

impl Error {
    /// creates a new error with the given context
    pub fn context<C>(cxt: C) -> Error
    where
        C: Into<String>,
    {
        Error {
            cxt: cxt.into(),
            src: None,
        }
    }

    /// creates a new error with the given context and source error
    pub fn context_source<C, S>(cxt: C, src: S) -> Error
    where
        C: Into<String>,
        S: Into<BoxDynError>,
    {
        Error {
            cxt: cxt.into(),
            src: Some(src.into()),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.cxt)
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response<Body> {
        log_prefix_error("response error", &self);

        error_response("internal server error")
    }
}

/// a helper trait that works similarly to anyhow::Context
pub trait Context<T, E> {
    fn context<C>(self, cxt: C) -> std::result::Result<T, Error>
    where
        C: Into<String>;
}

impl<T, E> Context<T, E> for std::result::Result<T, E>
where
    E: Into<BoxDynError>,
{
    fn context<C>(self, cxt: C) -> std::result::Result<T, Error>
    where
        C: Into<String>,
    {
        self.map_err(|err| Error {
            cxt: cxt.into(),
            src: Some(err.into()),
        })
    }
}

impl<T> Context<T, ()> for std::option::Option<T> {
    fn context<C>(self, cxt: C) -> std::result::Result<T, Error>
    where
        C: Into<String>,
    {
        self.ok_or(Error {
            cxt: cxt.into(),
            src: None,
        })
    }
}

/// logs the given mesage and error
///
/// will recursively print any errors that are contained inside the current
/// one.
pub fn log_prefix_error<D, E>(prefix: &D, err: &E)
where
    D: Display + ?Sized,
    E: std::error::Error,
{
    let mut msg = format!("0) {err}");
    let mut count = 1;
    let mut curr = std::error::Error::source(&err);

    while let Some(next) = curr {
        if let Err(err) = write!(&mut msg, "\n{count}) {next}") {
            tracing::error!("error when writing out error message {err}");

            return;
        }

        count += 1;
        curr = std::error::Error::source(next);
    }

    tracing::error!("{prefix}:\n{msg}");
}

/// attempts unwrap the result and will log the error if unsuccessful
pub fn prefix_try_result<D, T, E>(prefix: &D, given: Result<T, E>) -> Option<T>
where
    D: Display + ?Sized,
    E: std::error::Error,
{
    match given {
        Ok(rtn) => Some(rtn),
        Err(err) => {
            log_prefix_error(prefix, &err);

            None
        }
    }
}

/// wrapper method to just log an error
pub fn log_error<E>(err: &E)
where
    E: std::error::Error,
{
    log_prefix_error("error stack", err)
}

/*
macro_rules! simple_from {
    ($e:path) => {
        impl From<$e> for crate::error::api::Error {
            fn from(err: $e) -> Self {
                crate::error::Error::source("internal error", err)
            }
        }
    };
    ($e:path, $m:expr) => {
        impl From<$e> for crate::error::api::Error {
            fn from(err: $e) -> Self {
                crate::error::Error::source($m, err)
            }
        }
    }
}

pub(crate) use simple_from;
*/

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        Error::context("received Infallible?")
    }
}
