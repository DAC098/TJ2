use std::convert::Infallible;
use std::fmt::{Display, Result as FmtResult, Formatter, Write};

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{Response, IntoResponse};

pub type BoxDynError = Box<dyn std::error::Error + Send + Sync>;

pub fn error_response<S>(msg: S) -> Response<Body>
where
    S: Into<String>
{
    let message = msg.into();

    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("content-type", "text/plain; charset=utf-8")
        .header("content-length", message.len())
        .body(Body::from(message))
        .unwrap()
}

#[derive(Debug, thiserror::Error)]
pub struct Error {
    cxt: String,

    #[source]
    src: Option<BoxDynError>
}

impl Error {
    pub fn context<C>(cxt: C) -> Error
    where
        C: Into<String>
    {
        Error {
            cxt: cxt.into(),
            src: None,
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

pub trait Context<T, E> {
    fn context<C>(self, cxt: C) -> std::result::Result<T, Error>
    where
        C: Into<String>;
}

impl<T, E> Context<T, E> for std::result::Result<T, E>
where
    E: Into<BoxDynError>
{
    fn context<C>(self, cxt: C) -> std::result::Result<T, Error>
    where
        C: Into<String>
    {
        self.map_err(|err| Error {
            cxt: cxt.into(),
            src: Some(err.into())
        })
    }
}

impl<T> Context<T, ()> for std::option::Option<T> {
    fn context<C>(self, cxt: C) -> std::result::Result<T, Error>
    where
        C: Into<String>
    {
        self.ok_or(Error {
            cxt: cxt.into(),
            src: None
        })
    }
}

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

pub fn log_error<E>(err: &E)
where
    E: std::error::Error
{
    log_prefix_error("error stack", err)
}

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

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        Error::context("received Infallible?")
    }
}
