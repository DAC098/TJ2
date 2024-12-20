use std::io::{ErrorKind, Error as IoError};
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Poll, Context as TaskContext};

use futures::stream::{StreamExt, FuturesOrdered};
use pin_project::pin_project;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWrite;

use crate::error;
use crate::path::{add_extension, tokio_metadata};

#[derive(Debug, thiserror::Error)]
pub enum FileUpdaterError {
    #[error("the provided file has no file_name value")]
    NoFileName,

    #[error("the provided file was not found in the file system")]
    CurrNotFound,

    #[error("a previous update failed to be cleaned up and the previous file exists")]
    PrevExists,
    #[error("a previous update failed to be cleaned up and the temp file exists")]
    TempExists,

    #[error("the provided file path is not a file")]
    CurrNotFile,
    #[error("the previous file path is not a file")]
    PrevNotFile,

    #[error(transparent)]
    Io(#[from] std::io::Error)
}

#[pin_project]
pub struct FileUpdater {
    #[pin]
    file: File,
    curr: PathBuf,
    temp: PathBuf,
    prev: PathBuf,
}

impl FileUpdater {
    pub async fn new(path: PathBuf) -> Result<Self, FileUpdaterError> {
        // replace with path_add_extension when available
        let prev = add_extension(&path, "prev")
            .ok_or(FileUpdaterError::NoFileName)?;
        let temp = add_extension(&path, "temp")
            .unwrap();
        // end replace

        let (curr_check, prev_check) = tokio::join!(
            tokio_metadata(&path),
            tokio_metadata(&prev)
        );

        match curr_check {
            Ok(Some(meta)) => if !meta.is_file() {
                return Err(FileUpdaterError::CurrNotFile);
            }
            Ok(None) => return Err(FileUpdaterError::CurrNotFound),
            Err(err) => return Err(FileUpdaterError::Io(err)),
        }

        match prev_check {
            Ok(Some(meta)) => if !meta.is_file() {
                return Err(FileUpdaterError::PrevNotFile);
            } else {
                return Err(FileUpdaterError::PrevExists);
            }
            Ok(None) => {}
            Err(err) => return Err(FileUpdaterError::Io(err))
        }

        let result = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await;

        let file = match result {
            Ok(f) => f,
            Err(err) => match err.kind() {
                ErrorKind::AlreadyExists => return Err(FileUpdaterError::TempExists),
                _ => return Err(FileUpdaterError::Io(err))
            }
        };

        Ok(Self {
            file,
            curr: path,
            temp,
            prev
        })
    }

    pub async fn update(self) -> Result<UpdatedFile, UpdateError> {
        if let Err(err) = tokio::fs::rename(&self.curr, &self.prev).await {
            // no changes have been made to the file system so there is nothing
            // to recover from. just return the error
            return Err(UpdateError::PrevMove {
                temp: self.temp,
                err
            });
        }

        if let Err(err) = tokio::fs::rename(&self.temp, &self.curr).await {
            // the previous file has been moved so attempt to move the
            // previous file back to its original position
            if let Err(rcvr) = tokio::fs::rename(&self.prev, &self.curr).await {
                // the recovery failed and the previous file is not in its
                // original position. bail as there is nothing else to do
                Err(UpdateError::TempMoveRecovery {
                    prev: self.prev,
                    temp: self.temp,
                    err,
                    rcvr,
                })
            } else {
                Err(UpdateError::TempMove {
                    temp: self.temp,
                    err
                })
            }
        } else {
            Ok(UpdatedFile {
                curr: self.curr,
                prev: self.prev,
            })
        }
    }

    pub async fn clean(self) -> Result<(), (Self, std::io::Error)> {
        if let Err(err) = tokio::fs::remove_file(&self.temp).await {
            // similar to the rollback, nothing has happened so just return
            // the struct with the error
            Err((self, err))
        } else {
            // the previous file is now gone and all that is left is the
            // updated file
            Ok(())
        }
    }
}

impl AsyncWrite for FileUpdater {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, IoError>> {
        let pinned = self.project();

        pinned.file.poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), IoError>> {
        let pinned = self.project();

        pinned.file.poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), IoError>> {
        let pinned = self.project();

        pinned.file.poll_shutdown(cx)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("failed to move the current file to previous")]
    PrevMove {
        temp: PathBuf,
        err: std::io::Error
    },
    #[error("failed to move the temp file to current")]
    TempMove {
        temp: PathBuf,
        err: std::io::Error,
    },
    #[error("failed recovery after temp move error")]
    TempMoveRecovery {
        prev: PathBuf,
        temp: PathBuf,
        err: std::io::Error,
        rcvr: std::io::Error,
    }
}

#[derive(Debug)]
pub struct UpdatedFile {
    curr: PathBuf,
    prev: PathBuf,
}

impl UpdatedFile {
    pub async fn rollback(self) -> Result<(), (Self, std::io::Error)> {
        if let Err(err) = tokio::fs::rename(&self.prev, &self.curr).await {
            // since nothing has happened we can just return the struct with
            // the error
            Err((self, err))
        } else {
            // the previous file has been moved back to its original position
            // and the current version is now gone
            Ok(())
        }
    }

    pub async fn clean(self) -> Result<(), (Self, std::io::Error)> {
        if let Err(err) = tokio::fs::remove_file(&self.prev).await {
            // similar to the rollback, nothing has happened so just return
            // the struct with the error
            Err((self, err))
        } else {
            // the previous file is now gone and all that is left is the
            // updated file
            Ok(())
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemovedFileError {
    #[error("the provided file has no file_name value")]
    NoFileName,

    #[error("the provided file was not found in the file system")]
    CurrNotFound,

    #[error("a previous mark failed to be cleaned up and the marked file exists")]
    MarkExists,

    #[error(transparent)]
    Io(#[from] std::io::Error)
}

#[derive(Debug)]
pub struct RemovedFile {
    curr: PathBuf,
    mark: PathBuf,
}

impl RemovedFile {
    pub async fn mark(curr: PathBuf) -> Result<Self, RemovedFileError> {
        let mark = add_extension(&curr, "mark")
            .ok_or(RemovedFileError::NoFileName)?;

        let (curr_meta, mark_meta) = tokio::join!(
            tokio_metadata(&curr),
            tokio_metadata(&mark),
        );

        if curr_meta?.is_none() {
            return Err(RemovedFileError::CurrNotFound);
        }

        if mark_meta?.is_some() {
            return Err(RemovedFileError::MarkExists);
        }

        tokio::fs::rename(&curr, &mark).await?;

        Ok(Self {
            curr,
            mark
        })
    }

    pub async fn clean(self) -> Result<(), (Self, std::io::Error)> {
        if let Err(err) = tokio::fs::remove_file(&self.mark).await {
            Err((self, err))
        } else {
            Ok(())
        }
    }

    pub async fn rollback(self) -> Result<(), (Self, std::io::Error)> {
        if let Err(err) = tokio::fs::rename(&self.mark, &self.curr).await {
            Err((self, err))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub struct RemovedFiles {
    processed: Vec<RemovedFile>
}

impl RemovedFiles {
    pub fn new() -> Self {
        Self {
            processed: Vec::new()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.processed.is_empty()
    }

    pub async fn add(&mut self, to_drop: PathBuf) -> Result<(), RemovedFileError> {
        self.processed.push(RemovedFile::mark(to_drop).await?);

        Ok(())
    }

    pub async fn clean(self) -> Vec<(RemovedFile, std::io::Error)> {
        let mut futs = FuturesOrdered::new();

        for mark in self.processed {
            futs.push_back(mark.clean());
        }

        let mut failed = Vec::new();

        while let Some(result) = futs.next().await {
            if let Err(fail) = result {
                failed.push(fail);
            }
        }

        failed
    }

    pub async fn rollback(self) -> Vec<(RemovedFile, std::io::Error)> {
        let mut futs = FuturesOrdered::new();

        for mark in self.processed {
            futs.push_back(mark.rollback());
        }

        let mut failed = Vec::new();

        while let Some(result) = futs.next().await {
            if let Err(fail) = result {
                failed.push(fail);
            }
        }

        failed
    }

    pub async fn log_clean(self) {
        let failed = self.clean().await;

        for (marked, err) in failed {
            let prefix = format!(
                "failed to clean file: \"{}\"",
                marked.mark.display()
            );

            error::log_prefix_error(prefix.as_str(), &err);
        }
    }

    pub async fn log_rollback(self) {
        let failed = self.rollback().await;

        for (marked, err) in failed {
            let prefix = format!(
                "failed to rollback file: \"{}\"",
                marked.mark.display()
            );

            error::log_prefix_error(prefix.as_str(), &err);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreatedFileError {
    #[error("the provided file already exists")]
    AlreadyExists,

    #[error(transparent)]
    Io(#[from] std::io::Error)
}

#[derive(Debug)]
pub struct CreatedFile(PathBuf);

impl CreatedFile {
    pub async fn create(path: PathBuf) -> Result<Self, CreatedFileError> {
        let result = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await;

        if let Err(err) = result {
            Err(match err.kind() {
                ErrorKind::AlreadyExists => CreatedFileError::AlreadyExists,
                _ => CreatedFileError::Io(err)
            })
        } else {
            Ok(Self(path))
        }
    }

    pub async fn rollback(self) -> Result<(), (Self, std::io::Error)> {
        if let Err(err) = tokio::fs::remove_file(&self.0).await {
            Err((self, err))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub struct CreatedFiles {
    processed: Vec<CreatedFile>
}

impl CreatedFiles {
    pub fn new() -> Self {
        Self {
            processed: Vec::new()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.processed.is_empty()
    }

    pub async fn add(&mut self, path: PathBuf) -> Result<(), CreatedFileError> {
        self.processed.push(CreatedFile::create(path).await?);

        Ok(())
    }

    pub async fn rollback(self) -> Vec<(CreatedFile, std::io::Error)> {
        let mut futs = FuturesOrdered::new();

        for created in self.processed {
            futs.push_back(created.rollback());
        }

        let mut failed = Vec::new();

        while let Some(result) = futs.next().await {
            if let Err(fail) = result {
                failed.push(fail);
            }
        }

        failed
    }

    pub async fn log_rollback(self) {
        let failed = self.rollback().await;

        for (created, err) in failed {
            let prefix = format!(
                "failed to rollback file: \"{}\"",
                created.0.display()
            );

            error::log_prefix_error(prefix.as_str(), &err);
        }
    }
}
