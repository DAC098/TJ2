use std::path::{Path, PathBuf};
use std::fmt::{Display, Formatter};
use std::sync::OnceLock;

use crate::error::{self, Context};
use crate::path::{metadata, normalize_from};

pub trait TryDefault: Sized {
    type Error;

    fn try_default() -> Result<Self, Self::Error>;
}

static CWD: OnceLock<Box<Path>> = OnceLock::new();

pub fn get_cwd() -> Result<&'static Path, error::Error> {
    if let Some(cwd) = CWD.get() {
        return Ok(cwd);
    }

    let result = std::env::current_dir()
        .context("failed to retrieve the current working directory")?;

    if let Err(_) = CWD.set(result.into_boxed_path()) {
        Err(error::Error::context("failed to set cwd global"))
    } else {
        Ok(CWD.get().unwrap())
    }
}

#[derive(Debug)]
pub struct SrcFile<'a> {
    parent: &'a Path,
    src: &'a Path,
}

impl<'a> SrcFile<'a> {
    pub fn new(src: &'a Path) -> Result<Self, error::Error> {
        let parent = src.parent().context(format!(
            "failed to retrieve parent path from source file \"{}\"", src.display()
        ))?;

        Ok(SrcFile {
            parent,
            src
        })
    }

    pub fn normalize(&self, given: PathBuf) -> PathBuf {
        normalize_from(&self.parent, given)
    }
}

impl<'a> Display for SrcFile<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.src.display())
    }
}

pub struct Quote<'a>(pub &'a dyn Display);

impl<'a> Display for Quote<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.0)
    }
}

#[derive(Clone)]
pub struct DotPath<'a>(Vec<&'a dyn Display>);

impl<'a> DotPath<'a> {
    pub fn new(name: &'a (dyn Display)) -> Self {
        DotPath(vec![name])
    }

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

pub fn check_path<P>(given: P, src: &SrcFile<'_>, dot: DotPath<'_>, is_file: bool) -> Result<(), error::Error>
where
    P: AsRef<Path>
{
    let given_ref = given.as_ref();
    let path_display = given_ref.display();
    let path_quote = Quote(&path_display);

    let meta = metadata(given_ref).context(format!(
        "{dot} failed to retrieve metadata for {path_quote} in {src}"
    ))?.context(format!(
        "{dot} {path_quote} was not found {src}"
    ))?;

    if is_file {
        if !meta.is_file() {
            return Err(error::Error::context(format!(
                "{dot} {path_quote} is not a file in {src}"
            )));
        }
    } else {
        if !meta.is_dir() {
            return Err(error::Error::context(format!(
                "{dot} {path_quote} is not a directory in {src}"
            )));
        }
    }

    Ok(())
}
