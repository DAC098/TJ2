use std::str::FromStr;

use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use futures::{AsyncWrite, AsyncWriteExt, SinkExt, Stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio_util::compat::TokioAsyncWriteCompatExt;
use tokio_util::io::ReaderStream;

use crate::db;
use crate::db::ids::{EntryId, FileEntryId, JournalId};
use crate::fs::FileCreater;
use crate::journal::{
    assert_permission, sharing, FileEntry, Journal, PromoteOptions, ReceivedFile, RequestedFile,
};
use crate::net::body;
use crate::net::Error;
use crate::sec::authn::Initiator;
use crate::sec::authz::{Ability, Scope};
use crate::sec::hash::HashCheck;
use crate::sec::Hash;
use crate::state::{self, Storage};
use crate::stream::{CaptureTrailing, HashStream, MaxBytes, MaxBytesError};

use crate::router::handles::journals::entries::form::EntryFileForm;

#[derive(Debug, Deserialize)]
pub struct FileEntryPath {
    journals_id: JournalId,
    entries_id: EntryId,
    file_entry_id: FileEntryId,
}

#[derive(Debug, Deserialize)]
pub struct FileEntryQuery {
    download: Option<bool>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum RetrieveError {
    JournalNotFound,
    FileNotFound,
}

impl IntoResponse for RetrieveError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::FileNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn retrieve_file(
    state: state::SharedState,
    initiator: Initiator,
    Path(FileEntryPath {
        journals_id,
        entries_id,
        file_entry_id,
    }): Path<FileEntryPath>,
    Query(FileEntryQuery { download }): Query<FileEntryQuery>,
) -> Result<Response, Error<RetrieveError>> {
    let conn = state.db_conn().await?;

    let journal = Journal::retrieve(&conn, &journals_id)
        .await?
        .ok_or(Error::Inner(RetrieveError::JournalNotFound))?;

    assert_permission(
        &conn,
        &initiator,
        &journal,
        Scope::Entries,
        Ability::Read,
        sharing::Ability::EntryRead,
    )
    .await?;

    let file_entry = FileEntry::retrieve_file_entry(&conn, &entries_id, &file_entry_id)
        .await?
        .ok_or(Error::Inner(RetrieveError::FileNotFound))?;

    let received_file = file_entry
        .into_received()
        .map_err(|_| Error::Inner(RetrieveError::FileNotFound))?;

    let file_path = state
        .storage()
        .journal_file_entry(journal.id, received_file.id);
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .open(&file_path)
        .await?;
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
            format!("attachment; filename=\"{}\"", name),
        );
    }

    Ok(builder.body(Body::from_stream(reader))?)
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum UploadError {
    JournalNotFound,
    FileNotFound,
    NotRequestedFile,
    InvalidContentType,
    InvalidHash,
    TooLarge,
    TooSmall,
}

impl IntoResponse for UploadError {
    fn into_response(self) -> Response {
        match &self {
            Self::JournalNotFound | Self::FileNotFound => {
                (StatusCode::NOT_FOUND, body::Json(self)).into_response()
            }
            Self::NotRequestedFile
            | Self::InvalidContentType
            | Self::InvalidHash
            | Self::TooLarge
            | Self::TooSmall => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
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
    stream: Body,
) -> Result<body::Json<EntryFileForm>, Error<UploadError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let journal = Journal::retrieve(&transaction, &journals_id)
        .await?
        .ok_or(Error::Inner(UploadError::JournalNotFound))?;

    assert_permission(
        &transaction,
        &initiator,
        &journal,
        Scope::Entries,
        Ability::Update,
        sharing::Ability::EntryUpdate,
    )
    .await?;

    let file_entry = FileEntry::retrieve_file_entry(&transaction, &entries_id, &file_entry_id)
        .await?
        .ok_or(Error::Inner(UploadError::FileNotFound))?;

    let mime = get_mime(&headers)?;
    let hash_check =
        HashCheck::from_headers(&headers).map_err(|_| Error::Inner(UploadError::InvalidHash))?;

    let requested = file_entry
        .into_requested()
        .map_err(|_| Error::Inner(UploadError::NotRequestedFile))?;

    let record = create_file(
        state.storage(),
        transaction,
        journal,
        requested,
        mime,
        hash_check,
        stream,
    )
    .await?;

    Ok(body::Json(record.into()))
}

async fn create_file(
    storage: &Storage,
    conn: db::Transaction<'_>,
    journal: Journal,
    requested: RequestedFile,
    mime: mime::Mime,
    hash_check: HashCheck,
    stream: Body,
) -> Result<ReceivedFile, Error<UploadError>> {
    let file_path = storage.journal_file_entry(journal.id, requested.id);
    let mut compat = FileCreater::new(file_path).await?.compat_write();

    let write_result = write_body(stream.into_data_stream(), &mut compat, hash_check).await;

    let creater = compat.into_inner();

    let (written, hash) = match write_result {
        Ok(rtn) => rtn,
        Err(err) => {
            creater.log_clean().await;

            return Err(match err {
                WriteError::InvalidHash => Error::Inner(UploadError::InvalidHash),
                WriteError::TooLarge => Error::Inner(UploadError::TooLarge),
                WriteError::TooSmall => Error::Inner(UploadError::TooSmall),
                WriteError::Io(err) => Error::from(err),
                WriteError::Stream(err) => Error::from(err),
            });
        }
    };

    let options = PromoteOptions {
        mime,
        size: written,
        hash,
        created: Utc::now(),
    };

    let received = match requested.promote(&conn, options).await {
        Ok(rtn) => rtn,
        Err((_, err)) => {
            creater.log_clean().await;

            return Err(Error::from(err));
        }
    };

    let created = creater.create();

    if let Err(err) = conn.commit().await {
        created.log_rollback().await;

        Err(Error::from(err))
    } else {
        Ok(received)
    }
}

fn get_mime(headers: &HeaderMap) -> Result<mime::Mime, Error<UploadError>> {
    let content_type = headers
        .get("content-type")
        .ok_or(Error::Inner(UploadError::InvalidContentType))?
        .to_str()
        .map_err(|_| Error::Inner(UploadError::InvalidContentType))?;

    mime::Mime::from_str(&content_type).map_err(|_| Error::Inner(UploadError::InvalidContentType))
}

#[derive(Debug, thiserror::Error)]
enum WriteError<SE> {
    #[error("the calculated hash does not match")]
    InvalidHash,

    #[error("written bytes exceeds max")]
    TooLarge,

    #[error("not enough bytes were received to calculate hash")]
    TooSmall,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Stream(SE),
}

/// streams the [`Body`] into the given writer and calculates a hash with
/// number of bytes written
///
/// if specified this will do hash calculations against the data by comparing
/// the given [`Hash`], using the last 32 bytes of the stream as the [`Hash`],
/// or by just calculating the incoming data and doing no comparison
async fn write_body<'a, B, S, T, SE>(
    stream: S,
    writer: &'a mut T,
    hash_check: HashCheck,
) -> Result<(i64, Hash), WriteError<SE>>
where
    B: AsRef<[u8]>,
    S: Stream<Item = Result<B, SE>> + Unpin,
    T: AsyncWrite + Unpin,
{
    let mut hasher = blake3::Hasher::new();
    let mut max_bytes = MaxBytes::new(i64::MAX as usize);

    match hash_check {
        HashCheck::Given(given) => {
            HashStream::new(
                max_bytes.for_stream(stream).map_err(|err| match err {
                    MaxBytesError::MaxSize => WriteError::TooLarge,
                    MaxBytesError::Producer(p) => WriteError::Stream(p),
                }),
                &mut hasher,
            )
            .forward(writer.into_sink().sink_map_err(|e| WriteError::Io(e)))
            .await?;

            let hash = Hash::from(hasher);

            if given != hash {
                Err(WriteError::InvalidHash)
            } else {
                Ok((max_bytes.get_total().try_into().unwrap(), hash))
            }
        }
        HashCheck::AtEnd => {
            let mut trailing = CaptureTrailing::new(32);

            HashStream::new(
                max_bytes
                    .for_stream(trailing.for_stream(stream))
                    .map_err(|err| match err {
                        MaxBytesError::MaxSize => WriteError::TooLarge,
                        MaxBytesError::Producer(p) => WriteError::Stream(p),
                    }),
                &mut hasher,
            )
            .forward(writer.into_sink().sink_map_err(|e| WriteError::Io(e)))
            .await?;

            let hash = Hash::from(hasher);
            let given =
                Hash::from_slice(trailing.pop_occupied()).map_err(|_| WriteError::TooSmall)?;

            if hash == given {
                Ok((max_bytes.get_total().try_into().unwrap(), hash))
            } else {
                Err(WriteError::InvalidHash)
            }
        }
        HashCheck::None => {
            HashStream::new(
                max_bytes.for_stream(stream).map_err(|err| match err {
                    MaxBytesError::MaxSize => WriteError::TooLarge,
                    MaxBytesError::Producer(p) => WriteError::Stream(p),
                }),
                &mut hasher,
            )
            .forward(writer.into_sink().sink_map_err(|e| WriteError::Io(e)))
            .await?;

            Ok((max_bytes.get_total().try_into().unwrap(), hasher.into()))
        }
    }
}
