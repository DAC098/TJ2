use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;
use std::task::{Poll, Context as TaskContext};

use axum::body::Body;
use axum::extract::Path;
use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};
use chrono::{NaiveDate, Utc};
use futures::StreamExt;
use futures::stream::FuturesOrdered;
use pin_project::pin_project;
use serde::Deserialize;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_util::io::ReaderStream;

use crate::state;
use crate::db::ids::FileEntryId;
use crate::error::{self, Context};
use crate::fs::FileUpdater;
use crate::journal::{Journal, FileEntry};
use crate::router::body;
use crate::router::macros;
use crate::sec::authz::{Scope, Ability};

use super::auth;

#[pin_project]
struct WithFut<T, F> {
    attached: Option<T>,
    #[pin]
    future: F
}

impl<T, F> WithFut<T, F> {
    fn new(attached: T, future: F) -> Self {
        Self {
            attached: Some(attached),
            future,
        }
    }
}

impl<T, F, O> std::future::Future for WithFut<T, F>
where
    F: std::future::Future<Output = O>
{
    type Output = (Option<T>, O);

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let pinned = self.project();

        match pinned.future.poll(cx) {
            Poll::Ready(value) => {
                Poll::Ready((pinned.attached.take(), value))
            }
            Poll::Pending => Poll::Pending
        }
    }
}

pub async fn drop_files(list: Vec<PathBuf>) -> Vec<(PathBuf, std::io::Error)> {
    let mut futs = {
        let mut futs = FuturesOrdered::new();

        for path in list {
            let rm_fut = tokio::fs::remove_file(path.clone());

            futs.push_back(WithFut::new(path, rm_fut));
        }

        futs
    };

    let mut failed = Vec::new();

    while let Some((maybe_path, result)) = futs.next().await {
        let path = maybe_path.unwrap();

        if let Err(err) = result {
            failed.push((path, err));
        }
    }

    failed
}

pub async fn mark_remove(files_dir: &PathBuf, list: Vec<FileEntry>) -> Result<Vec<(PathBuf, PathBuf)>, (Vec<(PathBuf, PathBuf)>, std::io::Error)> {
    let mut successful = Vec::new();

    for entry in list {
        let curr = files_dir.join(format!("{}.file", entry.id));
        let mark = files_dir.join(format!("{}.file.rm", entry.id));

        if let Err(err) = tokio::fs::rename(&curr, &mark).await {
            return Err((successful, err));
        } else {
            successful.push((mark, curr));
        }
    }

    Ok(successful)
}

pub async fn unmark_remove(marked: Vec<(PathBuf, PathBuf)>) -> Vec<(PathBuf, std::io::Error)> {
    let mut futs = {
        let mut futs = FuturesOrdered::new();

        for (mark, curr) in marked {
            let re_fut = tokio::fs::rename(mark.clone(), curr.clone());

            futs.push_back(WithFut::new(curr, re_fut));
        }

        futs
    };

    let mut failed = Vec::new();

    while let Some((maybe_path, result)) = futs.next().await {
        let path = maybe_path.unwrap();

        if let Err(err) = result {
            failed.push((path, err));
        }
    }

    failed
}

pub async fn drop_marked(marked: Vec<(PathBuf, PathBuf)>) -> Vec<(PathBuf, std::io::Error)> {
    let mut futs = {
        let mut futs = FuturesOrdered::new();

        for (mark, _curr) in marked {
            let re_fut = tokio::fs::remove_file(mark.clone());

            futs.push_back(WithFut::new(mark, re_fut));
        }

        futs
    };

    let mut failed = Vec::new();

    while let Some((maybe_path, result)) = futs.next().await {
        let path = maybe_path.unwrap();

        if let Err(err) = result {
            failed.push((path, err));
        }
    }

    failed
}

#[derive(Debug, Deserialize)]
pub struct FileEntryPath {
    date: NaiveDate,
    file_entry_id: FileEntryId,
}

pub async fn retrieve_file(
    state: state::SharedState,
    headers: HeaderMap,
    Path(FileEntryPath { date, file_entry_id }): Path<FileEntryPath>,
) -> Result<Response, error::Error> {
    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator_pg!(&conn, &headers, None::<&'static str>);

    let result = Journal::retrieve_default_pg(&conn, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    auth::perm_check_pg!(&conn, initiator, journal, Scope::Entries, Ability::Read);

    let result = FileEntry::retrieve_file_entry_pg(&conn, &date, file_entry_id)
        .await
        .context("failed to retrieve journal entry file")?;

    let Some(file_entry) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let file_path = state.storage()
        .journal_file_entry(journal.id, file_entry.id);
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .open(&file_path)
        .await
        .context("failed to open file for journal file entry")?;
    let reader = ReaderStream::new(file);

    let mime = file_entry.get_mime();

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", mime.to_string())
        .header("content-length", file_entry.size)
        .body(Body::from_stream(reader))
        .context("failed to create file response")
}

pub async fn upload_file(
    state: state::SharedState,
    headers: HeaderMap,
    Path(FileEntryPath { date, file_entry_id }): Path<FileEntryPath>,
    stream: Body
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator_pg!(&transaction, &headers, None::<&'static str>);

    let result = Journal::retrieve_default_pg(&transaction, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        tracing::debug!("failed to find journal");

        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    auth::perm_check_pg!(&transaction, initiator, journal, Scope::Entries, Ability::Update);

    let result = FileEntry::retrieve_file_entry_pg(&transaction, &date, file_entry_id)
        .await
        .context("failed to retrieve journal entry file")?;

    let Some(mut file_entry) = result else {
        tracing::debug!("failed to find file entry");

        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let mime = get_mime(&headers)?;

    let file_path = state.storage()
        .journal_file_entry(journal.id, file_entry.id);
    let mut file_update = FileUpdater::new(file_path)
        .await
        .context("failed to create file updater")?;

    let (written, _hash) = match write_body(&mut file_update, stream).await {
        Ok(rtn) => rtn,
        Err(err) => {
            if let Err((_file_update, err)) = file_update.clean().await {
                error::log_prefix_error(
                    "failed to remove temp_path during upload",
                    &err
                );
            }

            return Err(error::Error::context_source(
                "failed to write request body to temp file",
                err
            ));
        }
    };

    file_entry.mime_type = get_mime_type(&mime);
    file_entry.mime_subtype = get_mime_subtype(&mime);
    file_entry.mime_param = get_mime_params(mime.params());
    file_entry.size = written;
    file_entry.updated = Some(Utc::now());

    // update the database record
    if let Err(err) = file_entry.update_pg(&transaction).await {
        if let Err((_file_update, clean_err)) = file_update.clean().await {
            error::log_prefix_error("failed to clean file update", &clean_err);
        }

        return Err(error::Error::context_source(
            "failed to update file_entries record",
            err
        ));
    }

    let updated = file_update.update()
        .await
        .context("failed to update file")?;

    // attempt to commit changes
    if let Err(err) = transaction.commit().await {
        if let Err((_updated, roll_err)) = updated.rollback().await {
            error::log_prefix_error("failed to rollback file changes", &roll_err);
        }

        return Err(error::Error::context_source(
            "failed to commit changes to file entry",
            err
        ));
    }

    if let Err((_updated, clean_err)) = updated.clean().await {
        error::log_prefix_error("failed to clean up file update", &clean_err);
    }

    Ok((
        StatusCode::OK,
        body::Json(file_entry)
    ).into_response())
}

async fn write_body<'a, T>(
    writer: &'a mut T,
    stream: Body,
) -> Result<(i64, blake3::Hash), error::Error>
where
    T: AsyncWrite + Unpin,
{
    let mut written: usize = 0;
    let mut hasher = blake3::Hasher::new();

    let mut stream = stream.into_data_stream();

    while let Some(result) = stream.next().await {
        let bytes = result
            .context("failed to get bytes from stream")?;
        let slice = bytes.as_ref();

        hasher.update(slice);

        let wrote = writer.write(slice)
            .await
            .context("failed to write bytes to stream")?;

        written = written.checked_add(wrote)
            .context("bytes written overflows usize")?;
    }

    writer.flush()
        .await
        .context("failed to flush contents of stream")?;

    let size = written.try_into()
        .context("failed to convert bytes written to i64")?;
    let hash = hasher.finalize();

    Ok((size, hash))
}

fn get_mime(headers: &HeaderMap) -> Result<mime::Mime, error::Error> {
    if let Some(value) = headers.get("content-type") {
        let content_type = value.to_str()
            .context("content-type contains invalid utf8 characters")?;

        mime::Mime::from_str(&content_type).context(
            "content-type is not a valid mime format"
        )
    } else {
        Err(error::Error::context("missing content-type header"))
    }
}

#[inline]
fn get_mime_type(mime: &mime::Mime) -> String {
    mime.type_()
        .as_str()
        .to_owned()
}

#[inline]
fn get_mime_subtype(mime: &mime::Mime) -> String {
    mime.subtype()
        .as_str()
        .to_owned()
}

fn get_mime_params(params: mime::Params<'_>) -> Option<String> {
    let collected = params.map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<String>>()
        .join(";");

    if !collected.is_empty() {
        Some(collected)
    } else {
        None
    }
}
