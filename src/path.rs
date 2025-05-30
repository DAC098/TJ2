use std::ffi::{OsStr, OsString};
use std::fs::Metadata;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

pub fn metadata<P>(path: P) -> Result<Option<Metadata>, std::io::Error>
where
    P: AsRef<Path>,
{
    match path.as_ref().metadata() {
        Ok(m) => Ok(Some(m)),
        Err(err) => match err.kind() {
            ErrorKind::NotFound => Ok(None),
            _ => Err(err),
        },
    }
}

pub async fn tokio_metadata<P>(path: P) -> Result<Option<Metadata>, std::io::Error>
where
    P: AsRef<Path>,
{
    match tokio::fs::metadata(path).await {
        Ok(m) => Ok(Some(m)),
        Err(err) => match err.kind() {
            ErrorKind::NotFound => Ok(None),
            _ => Err(err),
        },
    }
}

pub fn normalize_from<R, P>(root: R, path: P) -> PathBuf
where
    R: AsRef<Path>,
    P: AsRef<Path>,
{
    let mut components = path.as_ref().components();
    let mut rtn = PathBuf::new();

    let Some(first) = components.next() else {
        return rtn;
    };

    match first {
        Component::Prefix(prefix) => {
            rtn.push(prefix.as_os_str());
        }
        Component::ParentDir => {
            rtn.push(root);
            rtn.pop();
        }
        Component::Normal(c) => {
            rtn.push(root);
            rtn.push(c);
        }
        Component::RootDir => {
            rtn.push(first.as_os_str());
        }
        Component::CurDir => {
            rtn.push(root);
        }
    }

    for comp in components {
        match comp {
            Component::Prefix(prefix) => {
                rtn.push(prefix.as_os_str());
            }
            Component::ParentDir => {
                rtn.pop();
            }
            Component::Normal(c) => {
                rtn.push(c);
            }
            Component::RootDir => {
                rtn.push(comp.as_os_str());
            }
            Component::CurDir => {}
        }
    }

    rtn
}

pub fn add_extension<E>(path: &PathBuf, ext: E) -> Option<PathBuf>
where
    E: AsRef<OsStr>,
{
    add_ext(path, ext.as_ref())
}

pub fn add_ext(path: &PathBuf, ext: &OsStr) -> Option<PathBuf> {
    let file_name = match path.file_name() {
        None => return None,
        Some(value) => value,
    };

    let mut new_name = OsString::with_capacity(file_name.len() + ext.len() + 1);
    new_name.push(file_name);
    new_name.push(OsStr::new("."));
    new_name.push(ext);

    let mut copy = path.clone();
    copy.set_file_name(new_name);

    Some(copy)
}
