use std::fmt::{Display, Result as FmtResult, Formatter, Write};

pub type BoxDynError = Box<dyn std::error::Error + Send + Sync>;

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

pub fn print_error_stack(err: &Error) {
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

    tracing::error!("error stack:\n{msg}");
}
