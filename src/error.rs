use std::fmt::{Display, Result as FmtResult, Formatter, Write};

pub type BoxDynError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub struct Error {
    pub code: i32,
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
            code: 1,
            cxt: cxt.into(),
            src: None,
        }
    }

    pub fn code<C>(code: i32, cxt: C) -> Error
    where
        C: Into<String>
    {
        Error {
            code,
            cxt: cxt.into(),
            src: None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "CODE ({}) {}", self.code, self.cxt)
    }
}

pub trait Context<T, E> {
    fn context<C>(self, cxt: C) -> std::result::Result<T, Error>
    where
        C: Into<String>;

    fn code<C>(self, code: i32, cxt: C) -> std::result::Result<T, Error>
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
            code: 1,
            cxt: cxt.into(),
            src: Some(err.into())
        })
    }

    fn code<C>(self, code: i32, cxt: C) -> std::result::Result<T, Error>
    where
        C: Into<String>
    {
        self.map_err(|err| Error {
            code,
            cxt: cxt.into(),
            src: Some(err.into()),
        })
    }
}

impl<T> Context<T, ()> for std::option::Option<T> {
    fn context<C>(self, cxt: C) -> std::result::Result<T, Error>
    where
        C: Into<String>
    {
        self.ok_or(Error {
            code: 1,
            cxt: cxt.into(),
            src: None
        })
    }

    fn code<C>(self, code: i32, cxt: C) -> std::result::Result<T, Error>
    where
        C: Into<String>
    {
        self.ok_or(Error {
            code,
            cxt: cxt.into(),
            src: None,
        })
    }
}

pub fn print_error_stack(err: &Error) {
    let mut msg = format!("0) {err}");
    let mut count = 1;
    let mut curr = std::error::Error::source(&err);

    while let Some(next) = curr {
        if let Err(err) = write!(&mut msg, "\n{count}) {next}") {
            println!("error when writing out error message {err}");

            return;
        }

        count += 1;
        curr = std::error::Error::source(next);
    }

    println!("{msg}");
}
