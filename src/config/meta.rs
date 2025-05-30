//! traits and helpers when loading configuration files

use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::{self, Context};
use crate::path::{metadata, normalize_from};

/// works similar to Default but allows for defaults that can throw an error
pub trait TryDefault: Sized {
    /// the error type that can be returned by try_default
    type Error;

    /// the default values that a struct has that can potentially fail
    fn try_default() -> Result<Self, Self::Error>;
}

/// the current working directory of the server
static CWD: OnceLock<Box<Path>> = OnceLock::new();

/// retrieves that cached current working directory of the server
pub fn get_cwd() -> Result<&'static Path, error::Error> {
    if let Some(cwd) = CWD.get() {
        return Ok(cwd);
    }

    let result =
        std::env::current_dir().context("failed to retrieve the current working directory")?;

    if CWD.set(result.into_boxed_path()).is_err() {
        Err(error::Error::context("failed to set cwd global"))
    } else {
        Ok(CWD.get().unwrap())
    }
}

/// the paths of a config file
#[derive(Debug)]
pub struct SrcFile<'a> {
    /// the parent directory of the config file
    parent: &'a Path,

    /// the full path of the config file
    src: &'a Path,
}

impl<'a> SrcFile<'a> {
    /// creates the SrcFile from a reference to the given path.
    ///
    /// if it is unable to retrieve the parent directory of the current path
    /// this will fail
    pub fn new(src: &'a Path) -> Result<Self, error::Error> {
        let parent = src.parent().context(format!(
            "failed to retrieve parent path from source file \"{}\"",
            src.display()
        ))?;

        Ok(SrcFile { parent, src })
    }

    /// normalizes a given path using the parent directory of the src file
    pub fn normalize(&self, given: PathBuf) -> PathBuf {
        normalize_from(self.parent, given)
    }
}

impl<'a> Display for SrcFile<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.src.display())
    }
}

/// a helper struct for applying quotes to anything that implements the
/// Display trait
pub struct Quote<'a>(pub &'a dyn Display);

impl<'a> Display for Quote<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.0)
    }
}

/// a list of values that represent a dot path to a given object
#[derive(Clone)]
pub struct DotPath<'a>(Vec<&'a dyn Display>);

impl<'a> DotPath<'a> {
    /// creates a new DotPath with a single value
    pub fn new(name: &'a (dyn Display)) -> Self {
        DotPath(vec![name])
    }

    /// extends the current dot path with a new value and returning the new
    /// extended path.
    pub fn push(&self, name: &'a (dyn Display)) -> Self {
        let mut path = self.0.clone();
        path.push(name);

        DotPath(path)
    }
}

impl<'a> Display for DotPath<'a> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        let mut first = true;

        for name in &self.0 {
            if first {
                write!(fmt, "{name}")?;

                first = false;
            } else {
                write!(fmt, ".{name}")?;
            }
        }

        Ok(())
    }
}

/// checks to see if a given path exists as a file or a directory
///
/// the path exists if this returns without an error
pub fn check_path<P>(
    given: P,
    src: &SrcFile<'_>,
    dot: DotPath<'_>,
    is_file: bool,
) -> Result<(), error::Error>
where
    P: AsRef<Path>,
{
    let given_ref = given.as_ref();
    let path_display = given_ref.display();
    let path_quote = Quote(&path_display);

    let meta = metadata(given_ref)
        .context(format!(
            "{dot} failed to retrieve metadata for {path_quote} in {src}"
        ))?
        .context(format!("{dot} {path_quote} was not found {src}"))?;

    if is_file {
        if !meta.is_file() {
            return Err(error::Error::context(format!(
                "{dot} {path_quote} is not a file in {src}"
            )));
        }
    } else if !meta.is_dir() {
        return Err(error::Error::context(format!(
            "{dot} {path_quote} is not a directory in {src}"
        )));
    }

    Ok(())
}

/// sanitizes a given string as a url and returns the resulting string
pub fn sanitize_url_key(
    given: &str,
    src: &SrcFile<'_>,
    dot: DotPath<'_>,
) -> Result<String, error::Error> {
    let trimmed = given.trim();
    let rtn: String;

    let to_parse = if trimmed.starts_with('/') {
        rtn = trimmed.to_owned();

        format!("https://localhost{trimmed}")
    } else {
        rtn = format!("/{trimmed}");

        format!("https://localhost/{trimmed}")
    };

    let url = url::Url::parse(&to_parse).context(format!(
        "{dot} \"{given}\" is not a valid url path. file: {src}"
    ))?;

    for part in url.path_segments().unwrap() {
        if part == ".." || part == "." {
            return Err(error::Error::context(format!(
                "{dot} \"{given}\" is not a valid url path. file: {src}"
            )));
        }
    }

    Ok(rtn)
}
