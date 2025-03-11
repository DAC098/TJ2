use std::str::FromStr;

use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use futures::StreamExt;
use serde::{Serialize, Deserialize};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_util::io::ReaderStream;

use crate::state::{self, Storage};
use crate::db;
use crate::db::ids::{JournalId, EntryId, FileEntryId};
use crate::error::{self, Context};
use crate::fs::{FileUpdater, FileCreater};
use crate::journal::{
    Journal,
    FileEntry,
    PromoteOptions,
    RequestedFile,
    ReceivedFile
};
use crate::router::body;
use crate::sec::authn::Initiator;
use crate::sec::authz::{Scope, Ability};

use super::auth;
use super::EntryFileForm;

#[derive(Debug, Deserialize)]
pub struct FileEntryPath {
    journals_id: JournalId,
    entries_id: EntryId,
    file_entry_id: FileEntryId,
}

#[derive(Debug, Deserialize)]
pub struct FileEntryQuery {
    download: Option<bool>
}

pub async fn retrieve_file(
    state: state::SharedState,
    initiator: Initiator,
    Path(FileEntryPath {
        journals_id,
        entries_id,
        file_entry_id
    }): Path<FileEntryPath>,
    Query(FileEntryQuery {
        download
    }): Query<FileEntryQuery>,
) -> Result<Response, error::Error> {
    let conn = state.db_conn().await?;

    let result = Journal::retrieve_id(&conn, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    auth::perm_check!(&conn, initiator, journal, Scope::Entries, Ability::Read);

    let result = FileEntry::retrieve_file_entry(&conn, &entries_id, &file_entry_id)
        .await
        .context("failed to retrieve journal entry file")?;

    let Some(file_entry) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let Ok(received_file) = file_entry.into_received() else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let file_path = state.storage()
        .journal_file_entry(journal.id, received_file.id);
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .open(&file_path)
        .await
        .context("failed to open file for journal file entry")?;
    let reader = ReaderStream::new(file);

    let mime = received_file.get_mime();

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", mime.to_string())
        .header("content-length", received_file.size);

    if download.unwrap_or(false) {
        let name = received_file.name.unwrap_or(received_file.uid.into());

        builder = builder.header(
            "content-disposition",
            format!("attachment; filename=\"{}\"", name)
        );
    }

    builder.body(Body::from_stream(reader))
        .context("failed to create file response")
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum UploadResult {
    Successful(EntryFileForm),
    JournalNotFound,
    FileNotFound,
}

impl IntoResponse for UploadResult {
    fn into_response(self) -> Response {
        match &self {
            Self::Successful(_) => (
                StatusCode::OK,
                body::Json(self)
            ).into_response(),
            Self::JournalNotFound |
            Self::FileNotFound => (
                StatusCode::NOT_FOUND,
                body::Json(self)
            ).into_response()
        }
    }
}

pub async fn upload_file(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(FileEntryPath {
        journals_id,
        entries_id,
        file_entry_id,
    }): Path<FileEntryPath>,
    stream: Body
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let result = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(UploadResult::JournalNotFound.into_response());
    };

    auth::perm_check!(&transaction, initiator, journal, Scope::Entries, Ability::Update);

    let result = FileEntry::retrieve_file_entry(&transaction, &entries_id, &file_entry_id)
        .await
        .context("failed to retrieve journal entry file")?;

    let Some(file_entry) = result else {
        return Ok(UploadResult::FileNotFound.into_response());
    };

    let mime = get_mime(&headers)?;

    let record = match file_entry {
        FileEntry::Requested(requested) => {
            create_file(
                state.storage(),
                transaction,
                &journal,
                requested,
                mime,
                stream,
            )
                .await
                .context("failed to create file")?
        }
        FileEntry::Received(received) => {
            update_file(
                state.storage(),
                transaction,
                &journal,
                received,
                mime,
                stream,
            )
                .await
                .context("failed to update file")?
        }
    };

    Ok(UploadResult::Successful(record.into()).into_response())
}

async fn create_file(
    storage: &Storage,
    conn: db::Transaction<'_>,
    journal: &Journal,
    requested: RequestedFile,
    mime: mime::Mime,
    stream: Body,
) -> Result<ReceivedFile, error::Error> {
    let file_path = storage.journal_file_entry(journal.id, requested.id);
    let mut file_create = FileCreater::new(file_path)
        .await
        .context("failed to init file creater")?;

    let (written, _hash) = match write_body(&mut file_create, stream).await {
        Ok(rtn) => rtn,
        Err(err) => {
            file_create.log_clean().await;

            return Err(error::Error::context_source(
                "failed to write request body to file",
                err
            ));
        }
    };

    let options = PromoteOptions {
        mime,
        size: written,
        created: Utc::now()
    };

    let received = match requested.promote(&conn, options).await {
        Ok(rtn) => rtn,
        Err((_, err)) => {
            file_create.log_clean().await;

            return Err(error::Error::context_source(
                "failed to promote requested file entry",
                err
            ));
        }
    };

    let created = file_create.create();

    if let Err(err) = conn.commit().await {
        created.log_rollback().await;

        Err(error::Error::context_source(
            "failed to commit changes to file entry",
            err
        ))
    } else {
        Ok(received)
    }
}

async fn update_file(
    storage: &Storage,
    conn: db::Transaction<'_>,
    journal: &Journal,
    mut received: ReceivedFile,
    mime: mime::Mime,
    stream: Body,
) -> Result<ReceivedFile, error::Error> {
    let file_path = storage.journal_file_entry(journal.id, received.id);
    let mut file_update = FileUpdater::new(file_path)
        .await
        .context("failed to init file updater")?;

    let (written, _hash) = match write_body(&mut file_update, stream).await {
        Ok(rtn) => rtn,
        Err(err) => {
            file_update.log_clean().await;

            return Err(error::Error::context_source(
                "failed to write request body to file",
                err
            ));
        }
    };

    received.update_mime(&mime);
    received.size = written;
    received.updated = Some(Utc::now());

    if let Err(err) = received.update(&conn).await {
        file_update.log_clean().await;

        return Err(error::Error::context_source(
            "failed to update received file entry",
            err
        ));
    }

    let updated = file_update.update()
        .await
        .context("failed to update file")?;

    if let Err(err) = conn.commit().await {
        updated.log_rollback().await;

        Err(error::Error::context_source(
            "failed to commit changes to file entry",
            err
        ))
    } else {
        updated.log_clean().await;

        Ok(received)
    }
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
    let content_type = headers.get("content-type")
        .context("missing content-type header")?
        .to_str()
        .context("contet-type contains invalid utf8 characters")?;

    mime::Mime::from_str(&content_type)
        .context("content-type is not a valid mime format")
}
