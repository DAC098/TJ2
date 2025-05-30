use std::io::{Error as IoError, ErrorKind};
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};

use futures::stream::{FuturesOrdered, StreamExt};
use pin_project::pin_project;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWrite;

use crate::error;
use crate::path::{add_extension, tokio_metadata};

/// the possible error variants when working with a FileUpdater struct
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
    Io(#[from] std::io::Error),
}

/// helps to provide a transactional file to update without modifying the
/// contents of the current file.
///
/// this is a multistep process that will involve 3 different files.
///  - the "curr" file that will be updated
///  - the "temp" file that will be the new version of "curr"
///  - the "prev" file that will be the previous version of "curr"
///
/// as data is written to the object, "temp" will be updated with the changes.
/// when directed to, you can [`update`] the changes which have "curr" moved to
/// "prev" and "temp" moved to "curr". if the changes are to be disregarded
/// then use "clean" to delete changes and "curr" unmodified. once [`update`]
/// has been called, it will consume the updater and return an [`UpdatedFile`]
///
///
/// this will only work for files that are on the file system.
#[pin_project]
pub struct FileUpdater {
    /// the underlying File that will be written to
    #[pin]
    file: File,

    /// the file what will be updated when changes are committed
    curr: PathBuf,

    /// the temp file that will be written to before commiting to current
    temp: PathBuf,

    /// the previous version of the file that will be created when changes
    /// have been committed
    prev: PathBuf,
}

impl FileUpdater {
    pub async fn new(path: PathBuf) -> Result<Self, FileUpdaterError> {
        // replace with path_add_extension when available
        let prev = add_extension(&path, "prev").ok_or(FileUpdaterError::NoFileName)?;
        let temp = add_extension(&path, "temp").unwrap();
        // end replace

        let (curr_check, prev_check) = tokio::join!(tokio_metadata(&path), tokio_metadata(&prev));

        match curr_check {
            Ok(Some(meta)) => {
                if !meta.is_file() {
                    return Err(FileUpdaterError::CurrNotFile);
                }
            }
            Ok(None) => return Err(FileUpdaterError::CurrNotFound),
            Err(err) => return Err(FileUpdaterError::Io(err)),
        }

        match prev_check {
            Ok(Some(meta)) => {
                if !meta.is_file() {
                    return Err(FileUpdaterError::PrevNotFile);
                } else {
                    return Err(FileUpdaterError::PrevExists);
                }
            }
            Ok(None) => {}
            Err(err) => return Err(FileUpdaterError::Io(err)),
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
                _ => return Err(FileUpdaterError::Io(err)),
            },
        };

        Ok(Self {
            file,
            curr: path,
            temp,
            prev,
        })
    }

    /// attempst to update the current file with new data written into "temp"
    pub async fn update(self) -> Result<UpdatedFile, UpdateError> {
        if let Err(err) = tokio::fs::rename(&self.curr, &self.prev).await {
            // no changes have been made to the file system so there is nothing
            // to recover from. just return the error
            return Err(UpdateError::PrevMove {
                temp: self.temp,
                err,
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
                    err,
                })
            }
        } else {
            Ok(UpdatedFile {
                curr: self.curr,
                prev: self.prev,
            })
        }
    }

    /// attempts to remove "temp" and consume self
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

    pub async fn log_clean(self) {
        if let Err((updater, err)) = self.clean().await {
            let prefix = format!("failed to clean temp file: \"{}\"", updater.temp.display());

            error::log_prefix_error(&prefix, &err);
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

    fn poll_flush(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Result<(), IoError>> {
        let pinned = self.project();

        pinned.file.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Result<(), IoError>> {
        let pinned = self.project();

        pinned.file.poll_shutdown(cx)
    }
}

/// the potential errors that can arise when updating a file
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    /// failed to move the current file to previous
    #[error("failed to move the current file to previous")]
    PrevMove { temp: PathBuf, err: std::io::Error },

    /// failed to move the temp file to current
    #[error("failed to move the temp file to current")]
    TempMove { temp: PathBuf, err: std::io::Error },

    /// failed recovery after temp move error
    #[error("failed recovery after temp move error")]
    TempMoveRecovery {
        prev: PathBuf,
        temp: PathBuf,
        err: std::io::Error,
        rcvr: std::io::Error,
    },
}

/// the resulting files after a file has been updated
#[derive(Debug)]
pub struct UpdatedFile {
    curr: PathBuf,
    prev: PathBuf,
}

impl UpdatedFile {
    /// attempts to rollback the changes of an update by moving "prev" back
    /// to "curr"
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

    pub async fn log_rollback(self) {
        if let Err((updated, err)) = self.rollback().await {
            let prefix = format!(
                "failed to rollback updated file: \"{}\"",
                updated.curr.display(),
            );

            error::log_prefix_error(&prefix, &err);
        }
    }

    /// attempts to remove "prev"
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

    pub async fn log_clean(self) {
        if let Err((updated, err)) = self.clean().await {
            let prefix = format!(
                "failed to cleanup updated file: \"{}\"",
                updated.prev.display()
            );

            error::log_prefix_error(&prefix, &err);
        }
    }
}

/// the potential errors that arise when removing a file
#[derive(Debug, thiserror::Error)]
pub enum RemovedFileError {
    /// the provided file has no file_name value
    #[error("the provided file has no file_name value")]
    NoFileName,

    /// the provided file was not found in the file system
    #[error("the provided file was not found in the file system")]
    CurrNotFound,

    /// a previous mark failed to be cleaned up and the marked file exists
    #[error("a previous mark failed to be cleaned up and the marked file exists")]
    MarkExists,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// represents a file that has been marked for deletion.
///
/// similar to the [`UpdatedFile`] in that the file has not yet been deleted
/// and can still be recovered. it has only been "mark"ed
#[derive(Debug)]
pub struct RemovedFile {
    curr: PathBuf,
    mark: PathBuf,
}

impl RemovedFile {
    /// marks the specified file for deletion
    pub async fn mark(curr: PathBuf) -> Result<Self, RemovedFileError> {
        let mark = add_extension(&curr, "mark").ok_or(RemovedFileError::NoFileName)?;

        let (curr_meta, mark_meta) = tokio::join!(tokio_metadata(&curr), tokio_metadata(&mark),);

        if curr_meta?.is_none() {
            return Err(RemovedFileError::CurrNotFound);
        }

        if mark_meta?.is_some() {
            return Err(RemovedFileError::MarkExists);
        }

        tokio::fs::rename(&curr, &mark).await?;

        Ok(Self { curr, mark })
    }

    /// attempts to remove the marked file
    pub async fn clean(self) -> Result<(), (Self, std::io::Error)> {
        if let Err(err) = tokio::fs::remove_file(&self.mark).await {
            Err((self, err))
        } else {
            Ok(())
        }
    }

    /// attempts to recover the marked file
    pub async fn rollback(self) -> Result<(), (Self, std::io::Error)> {
        if let Err(err) = tokio::fs::rename(&self.mark, &self.curr).await {
            Err((self, err))
        } else {
            Ok(())
        }
    }
}

/// contains a list of files marked for deletion
#[derive(Debug)]
pub struct RemovedFiles {
    processed: Vec<RemovedFile>,
}

impl RemovedFiles {
    /// creates an empty RemovedFiles struct
    pub fn new() -> Self {
        Self {
            processed: Vec::new(),
        }
    }

    /// checks to see if any files have been processed
    pub fn is_empty(&self) -> bool {
        self.processed.is_empty()
    }

    /// attempts to mark a file for deletion.
    ///
    /// if the file is failed to be marked then an error will be returned
    pub async fn add(&mut self, to_drop: PathBuf) -> Result<(), RemovedFileError> {
        self.processed.push(RemovedFile::mark(to_drop).await?);

        Ok(())
    }

    /// attempts to remove all marked files
    ///
    /// in the event that a file was not removed, a list of failed files will
    /// be returned along with the associated error that caused the failure.
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

    /// attempts to restore all marked files
    ///
    /// in the event that a files was not restored, a list of failed files will
    /// be returned along with the associated error that caused the failure.
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

    /// attempts to remove all marked files and logs any failures
    pub async fn log_clean(self) {
        let failed = self.clean().await;

        for (marked, err) in failed {
            let prefix = format!("failed to clean file: \"{}\"", marked.mark.display());

            error::log_prefix_error(prefix.as_str(), &err);
        }
    }

    /// attempts to restore all marked files and logs any failures
    pub async fn log_rollback(self) {
        let failed = self.rollback().await;

        for (marked, err) in failed {
            let prefix = format!("failed to rollback file: \"{}\"", marked.mark.display());

            error::log_prefix_error(prefix.as_str(), &err);
        }
    }
}

/// the potential errors when creating a file
#[derive(Debug, thiserror::Error)]
pub enum FileCreaterError {
    /// the provided file already exists
    #[error("the provided file already exists")]
    AlreadyExists,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[pin_project]
pub struct FileCreater {
    /// the underlying File that will be written to
    #[pin]
    file: File,

    /// the file what will be updated when changes are committed
    curr: PathBuf,
}

impl FileCreater {
    pub async fn new(path: PathBuf) -> Result<Self, FileCreaterError> {
        let result = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await;

        match result {
            Ok(file) => Ok(Self { file, curr: path }),
            Err(err) => Err(match err.kind() {
                ErrorKind::AlreadyExists => FileCreaterError::AlreadyExists,
                _ => FileCreaterError::Io(err),
            }),
        }
    }

    /// attempts to create the specified file
    pub fn create(self) -> CreatedFile {
        CreatedFile(self.curr)
    }

    /// attempts to remove "curr" and consume self
    pub async fn clean(self) -> Result<(), (Self, std::io::Error)> {
        if let Err(err) = tokio::fs::remove_file(&self.curr).await {
            Err((self, err))
        } else {
            Ok(())
        }
    }

    pub async fn log_clean(self) {
        if let Err((created, err)) = self.clean().await {
            let prefix = format!(
                "failed to clean up created file: \"{}\"",
                created.curr.display()
            );

            error::log_prefix_error(&prefix, &err);
        }
    }
}

impl AsyncWrite for FileCreater {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, IoError>> {
        let pinned = self.project();

        pinned.file.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Result<(), IoError>> {
        let pinned = self.project();

        pinned.file.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Result<(), IoError>> {
        let pinned = self.project();

        pinned.file.poll_shutdown(cx)
    }
}

/// similar to [`RemovedFile`] except will only create a single file
#[derive(Debug)]
pub struct CreatedFile(PathBuf);

impl CreatedFile {
    /// attempts to remove the created file
    pub async fn rollback(self) -> Result<(), (Self, std::io::Error)> {
        if let Err(err) = tokio::fs::remove_file(&self.0).await {
            Err((self, err))
        } else {
            Ok(())
        }
    }

    pub async fn log_rollback(self) {
        if let Err((created, err)) = self.rollback().await {
            let prefix = format!(
                "failed to rollback created file: \"{}\"",
                created.0.display()
            );

            error::log_prefix_error(&prefix, &err);
        }
    }
}
