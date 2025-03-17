use std::str::FromStr;

use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use futures::StreamExt;
use ringbuf::traits::{Producer, Consumer, Observer};
use serde::{Serialize, Deserialize};
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};
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
use crate::sec::Hash;
use crate::sec::hash::HashCheck;
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
    NotRequestedFile,
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
            ).into_response(),
            Self::NotRequestedFile => (
                StatusCode::BAD_REQUEST,
                body::Json(self)
            ).into_response(),
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
    let hash_check = HashCheck::from_headers(&headers)
        .context("error retrieving x-hash header")?;

    let Ok(requested) = file_entry.into_requested() else {
        return Ok(UploadResult::NotRequestedFile.into_response());
    };

    let record = create_file(
        state.storage(),
        transaction,
        journal,
        requested,
        mime,
        hash_check,
        stream,
    )
        .await
        .context("failed to create file")?;

    Ok(UploadResult::Successful(record.into()).into_response())
}

async fn create_file(
    storage: &Storage,
    conn: db::Transaction<'_>,
    journal: Journal,
    requested: RequestedFile,
    mime: mime::Mime,
    hash_check: HashCheck,
    stream: Body,
) -> Result<ReceivedFile, error::Error> {
    let file_path = storage.journal_file_entry(journal.id, requested.id);
    let mut creater = FileCreater::new(file_path)
        .await
        .context("failed to init file creater")?;

    let (written, hash) = match write_body(stream, &mut creater, hash_check).await {
        Ok(rtn) => rtn,
        Err(err) => {
            creater.log_clean().await;

            return Err(error::Error::context_source(
                "failed to write request body to file",
                err
            ));
        }
    };

    let options = PromoteOptions {
        mime,
        size: written,
        hash,
        created: Utc::now()
    };

    let received = match requested.promote(&conn, options).await {
        Ok(rtn) => rtn,
        Err((_, err)) => {
            creater.log_clean().await;

            return Err(error::Error::context_source(
                "failed to promote requested file entry",
                err
            ));
        }
    };

    let created = creater.create();

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

#[derive(Debug, thiserror::Error)]
enum WriteError {
    #[error("the calculated hash does not match")]
    InvalidHash,

    #[error("written bytes exceeds max")]
    TooLarge,

    #[error("not enough bytes were received to calculate hash")]
    TooSmall,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Axum(#[from] axum::Error),
}

const BUF_SIZE: usize = 8 * 1024;

/// streams the [`Body`] into the given writer and calculates a hash with
/// number of bytes written
///
/// if specified this will do hash calculations against the data by comparing
/// the given [`Hash`], using the last 32 bytes of the stream as the [`Hash`],
/// or by just calculating the incoming data and doing no comparison
async fn write_body<'a, T>(
    stream: Body,
    writer: &'a mut T,
    hash_check: HashCheck,
) -> Result<(i64, Hash), WriteError>
where
    T: AsyncWrite + Unpin,
{
    match hash_check {
        HashCheck::Given(given) => {
            let (size, hash) = stream_to_writer(stream, writer).await?;

            if given != hash {
                Err(WriteError::InvalidHash)
            } else {
                Ok((size, hash))
            }
        }
        HashCheck::AtEnd => {
            let (size, hash, given) = stream_to_writer_truncate(stream, writer).await?;

            if hash == given {
                Ok((size, hash.into()))
            } else {
                Err(WriteError::InvalidHash)
            }
        }
        HashCheck::None => {
            stream_to_writer(stream, writer).await
        }
    }
}

/// streams the [`Body`] into the given writer and calculates a hash with
/// number of bytes written
async fn stream_to_writer<'a, T>(
    stream: Body,
    writer: &'a mut T,
) -> Result<(i64, Hash), WriteError>
where
    T: AsyncWrite + Unpin,
{
    let mut written = 0usize;
    let mut hasher = blake3::Hasher::new();
    let mut buf_writer = BufWriter::with_capacity(BUF_SIZE, &mut *writer);
    let mut stream = stream.into_data_stream();

    while let Some(result) = stream.next().await {
        let bytes = result?;
        let slice = bytes.as_ref();

        hasher.update(slice);

        let wrote = buf_writer.write(slice).await?;

        written = written.checked_add(wrote)
            .ok_or(WriteError::TooLarge)?;
    }

    buf_writer.flush().await?;

    let hash = hasher.finalize();
    let size = written.try_into()
        .map_err(|_| WriteError::TooLarge)?;

    Ok((size, hash.into()))
}

/// streams the [`Body`] into the given writer and calculates a hash with
/// number of bytes writen.
///
/// will truncate the last 32 bytes from the stream for use as the provided
/// hash of the data.
async fn stream_to_writer_truncate<'a, T>(
    stream: Body,
    writer: &'a mut T,
) -> Result<(i64, Hash, Hash), WriteError>
where
    T: AsyncWrite + Unpin,
{
    let mut written = 0usize;
    let mut hasher = blake3::Hasher::new();
    let mut stream = stream.into_data_stream();
    // allocate enough memory for a reasonable write size and the size of the blake3 hash
    let mut ring_buf = ringbuf::StaticRb::<u8, { BUF_SIZE + 32 }>::default();
    let mut buffer = [0u8; BUF_SIZE];

    tracing::trace!("buffer size: {} ring size: {}", buffer.len(), ring_buf.vacant_len());

    while let Some(result) = stream.next().await {
        let bytes = result?;
        let mut slice = bytes.as_ref();

        loop {
            let pushed = ring_buf.push_slice(slice);

            tracing::trace!("pushing slice to buffer. size: {} pushed: {pushed}", slice.len());

            if pushed == slice.len() {
                break;
            }

            // there is still data in the slice that we did not push into the
            // ring buffer. take out 8k and send to the writer
            let popped = ring_buf.pop_slice(&mut buffer);

            tracing::trace!("pushing to output. popped: {popped}");

            hasher.update(&buffer);
            writer.write_all(&buffer).await?;

            written = written.checked_add(buffer.len())
                .ok_or(WriteError::TooLarge)?;

            // create a sub slice of the data that was pushed
            slice = &slice[pushed..];
        }
    }

    let occupied_len = ring_buf.occupied_len();

    // take any remaing data in the ringbuffer except for the last 32 bytes
    if occupied_len > 32 {
        let diff = occupied_len - 32;
        let slice = &mut buffer[..diff];

        tracing::trace!("wrting remaining data to output. occupied_len: {occupied_len} diff: {diff}");

        ring_buf.pop_slice(slice);

        hasher.update(slice);
        writer.write_all(slice).await?;

        written = written.checked_add(slice.len())
            .ok_or(WriteError::TooLarge)?;
    }

    writer.flush().await?;

    // we did not receive any data that would not consider the hash at the end
    if written == 0 {
        return Err(WriteError::TooSmall);
    }

    let given = {
        let mut hash_buf = [0u8; 32];

        ring_buf.pop_slice(&mut hash_buf);

        blake3::Hash::from_bytes(hash_buf)
    };

    let hash = hasher.finalize();
    let size = written.try_into()
        .map_err(|_| WriteError::TooLarge)?;

    Ok((size, hash.into(), given.into()))
}

fn get_mime(headers: &HeaderMap) -> Result<mime::Mime, error::Error> {
    let content_type = headers.get("content-type")
        .context("missing content-type header")?
        .to_str()
        .context("contet-type contains invalid utf8 characters")?;

    mime::Mime::from_str(&content_type)
        .context("content-type is not a valid mime format")
}

#[cfg(test)]
mod test {
    use crate::sec::hash::{Hash, HashCheck};
    use super::*;

    fn gen_bytes(amount: usize) -> Vec<u8> {
        let mut rtn = Vec::with_capacity(amount);
        let marker = u8::MAX as usize;

        for count in 0..amount {
            if count % marker == 0 {
                rtn.push(u8::MAX);
            } else {
                rtn.push((count % 10) as u8);
            }
        }

        rtn
    }

    async fn run_write(amount: usize) {
        let data = gen_bytes(amount);
        let expected_hash = blake3::hash(&data);

        let mut output = Vec::new();
        let stream = {
            let mut stream_data = data.clone();
            stream_data.extend_from_slice(expected_hash.as_bytes());

            axum::body::Body::from(stream_data)
        };

        match stream_to_writer_truncate(stream, &mut output).await {
            Ok((size, hash, given)) => {
                assert_eq!(size as usize, amount, "unexpected amout of written bytes");
                assert_eq!(hash, given, "hash mismatch");
                assert_eq!(output, data, "output data does not match input");
            }
            Err(err) => panic!("failed to stream to output: {err}")
        }
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn write_body_10() {
        run_write(10).await;
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn write_body_1_000() {
        run_write(1_000).await;
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn write_body_10_000() {
        run_write(10_000).await;
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn write_body_100_000() {
        run_write(100_000).await;
    }
}
