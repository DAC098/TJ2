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
        trace_error!("response error", &self);

        error_response("internal server error")
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        Error::context("received Infallible?")
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

/// creates an error stack string from the provided error.
pub fn create_error_list<D, E>(prefix: &D, err: &E) -> Result<String, std::fmt::Error>
where
    D: Display + ?Sized,
    E: std::error::Error + ?Sized,
{
    let mut msg = format!("{prefix}: \n0) {err}");
    let mut count = 1;
    let mut curr = std::error::Error::source(&err);

    loop {
        let Some(next) = curr else {
            return Ok(msg);
        };

        msg.write_fmt(format_args!("\n{count}) {next}"))?;

        count += 1;
        curr = std::error::Error::source(next);
    }
}

/// logs the given mesage and error
///
/// favor [`trace_error!`] as it will properly log the line the trace is
/// called at
#[deprecated = "favor `trace_error` as it will provide better logging data"]
pub fn log_prefix_error<D, E>(prefix: &D, err: &E)
where
    D: Display + ?Sized,
    E: std::error::Error + ?Sized,
{
    match create_error_list(prefix, err) {
        Ok(msg) => tracing::error!("{msg}"),
        Err(fmt_err) => tracing::error!("failed to create error stack: {fmt_err}\noriginal: {err}"),
    }
}

/// similar to [`log_prefix_error`] but with a prefined message prefix of
/// "error stack"
///
/// favor [`trace_error!`] as it will properly log the line the trace is
/// called at
#[deprecated = "favor `trace_error` as it will provide better logging data"]
pub fn log_error<E>(err: &E)
where
    E: std::error::Error,
{
    match create_error_list("error stack", err) {
        Ok(msg) => tracing::error!("{msg}"),
        Err(fmt_err) => tracing::error!("failed to create error stack: {fmt_err}\noriginal: {err}"),
    }
}

/// logs a given error and optional prefix message using [`tracing::error!`]
macro_rules! trace_error {
    ($prefix:expr, $err:expr) => {
        match crate::error::create_error_list($prefix, $err) {
            Ok(msg) => tracing::error!("{msg}"),
            Err(fmt_err) => tracing::error!(
                "failed to create error stack: {fmt_err}\noriginal: {}",
                $err
            ),
        }
    };
    ($err:expr) => {
        match crate::error::create_error_list("error stack", $err) {
            Ok(msg) => tracing::error!("{msg}"),
            Err(fmt_err) => tracing::error!(
                "failed to create error stack: {fmt_err}\noriginal: {}",
                $err
            ),
        }
    };
}

pub(crate) use trace_error;

macro_rules! trace_pass {
    ($prefix:expr, $value:expr) => {
        match $value {
            Ok(v) => Ok(v),
            Err(err) => {
                crate::error::trace_error!($prefix, &err);

                Err(err)
            }
        }
    };
    ($value:expr) => {
        match $value {
            Ok(v) => Ok(v),
            Err(err) => {
                crate::error::trace_error!(&err);

                Err(err)
            }
        }
    };
}

pub(crate) use trace_pass;

macro_rules! trace_and_exit {
    ($value:expr, $prefix:expr, $code:expr) => {
        match $value {
            Ok(v) => v,
            Err(err) => {
                crate::error::trace_error!($prefix, &err);

                std::process::exit($code);
            }
        }
    };
    ($value:expr, $prefix:expr) => {
        match $value {
            Ok(v) => v,
            Err(err) => {
                crate::error::trace_error!($prefix, &err);

                std::process::exit(1);
            }
        }
    };
    ($value:expr) => {
        match $value {
            Ok(v) => v,
            Err(err) => {
                crate::error::trace_error!(&err);

                std::process::exit(1);
            }
        }
    };
}

pub(crate) use trace_and_exit;
