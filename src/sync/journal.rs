use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bytes::BytesMut;
use chrono::{DateTime, NaiveDate, Utc};
use futures::{Stream, StreamExt};
use postgres_types as pg_types;
use serde::{Deserialize, Serialize};

use crate::db::ids::{
    CustomFieldUid, EntryId, EntryUid, FileEntryUid, JournalId, JournalUid, UserPeerId, UserUid,
};
use crate::db::{GenericClient, ParamsArray};
use crate::error::{self, BoxDynError, Context};
use crate::journal::{custom_field, FileStatus};
use crate::router::body;
use crate::sec::Hash;

#[derive(Debug, strum::Display, Serialize, Deserialize)]
#[serde(tag = "error")]
pub enum SyncEntryError {
    JournalNotFound,
    NotRemoteJournal,
    UserNotFound,
    CFNotFound { uids: Vec<CustomFieldUid> },
    CFInvalid { uids: Vec<CustomFieldUid> },
    FileNotFound { uids: Vec<FileEntryUid> },
}

impl IntoResponse for SyncEntryError {
    fn into_response(self) -> Response {
        match &self {
            Self::JournalNotFound | Self::UserNotFound => {
                (StatusCode::NOT_FOUND, body::Json(self)).into_response()
            }
            Self::NotRemoteJournal
            | Self::CFNotFound { .. }
            | Self::CFInvalid { .. }
            | Self::FileNotFound { .. } => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncJournalResult {
    Synced,
    UserNotFound,
}

impl IntoResponse for SyncJournalResult {
    fn into_response(self) -> Response {
        match &self {
            Self::Synced => (StatusCode::CREATED, body::Json(self)).into_response(),
            Self::UserNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

#[derive(Debug)]
pub enum SyncStatus {
    Synced = 0,
    Failed = 1,
}

#[derive(Debug, thiserror::Error)]
#[error("the provided status value is invalid")]
pub struct InvalidStatus;

impl From<&SyncStatus> for i16 {
    fn from(value: &SyncStatus) -> Self {
        match value {
            SyncStatus::Synced => 0,
            SyncStatus::Failed => 1,
        }
    }
}

impl TryFrom<i16> for SyncStatus {
    type Error = InvalidStatus;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Synced),
            1 => Ok(Self::Failed),
            _ => Err(InvalidStatus),
        }
    }
}

impl<'a> pg_types::FromSql<'a> for SyncStatus {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let v = <i16 as pg_types::FromSql>::from_sql(ty, raw)?;

        Self::try_from(v).map_err(Into::into)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i16 as pg_types::FromSql>::accepts(ty)
    }
}

impl pg_types::ToSql for SyncStatus {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
        let v: i16 = self.into();

        v.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i16 as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JournalSync {
    pub uid: JournalUid,
    pub name: String,
    pub description: Option<String>,
    pub custom_fields: Vec<CustomFieldSync>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomFieldSync {
    pub uid: CustomFieldUid,
    pub name: String,
    pub order: i32,
    pub config: custom_field::Type,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntrySync {
    pub uid: EntryUid,
    pub journals_uid: JournalUid,
    pub users_uid: UserUid,
    pub date: NaiveDate,
    pub title: Option<String>,
    pub contents: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
    pub tags: Vec<EntryTagSync>,
    pub custom_fields: Vec<EntryCFSync>,
    pub files: Vec<EntryFileSync>,
}

impl EntrySync {
    pub async fn retrieve_batch_stream(
        conn: &impl GenericClient,
        journals_id: &JournalId,
        user_peers_id: &UserPeerId,
        prev_entry: &EntryId,
        sync_date: &DateTime<Utc>,
        batch_size: i64,
    ) -> Result<impl Stream<Item = Result<(EntryId, Self), error::Error>>, error::Error> {
        let params: ParamsArray<5> = [
            journals_id,
            user_peers_id,
            prev_entry,
            sync_date,
            &batch_size,
        ];
        let query = "\
            select entries.id, \
                   entries.uid, \
                   journals.uid, \
                   users.uid, \
                   entries.entry_date, \
                   entries.title, \
                   entries.contents, \
                   entries.created, \
                   entries.updated \
            from entries \
                left join users on \
                    entries.users_id = users.id \
                left join journals on \
                    entries.journals_id = journals.id \
                left join synced_entries on \
                    entries.id = synced_entries.entries_id and \
                    synced_entries.user_peers_id = $2 \
            where entries.journals_id = $1 and \
                  entries.id > $3 and ( \
                      synced_entries.status is null or ( \
                          (
                              synced_entries.updated < ( \
                                  case when entries.updated is null \
                                      then entries.created \
                                      else entries.updated \
                                      end \
                              ) or \
                              synced_entries.status = 1 \
                          ) and \
                          synced_entries.updated < $4 \
                      ) \
                  ) \
            order by entries.id \
            limit $5";

        let stream = conn
            .query_raw(query, params)
            .await
            .context("failed to retrieve entries batch")?;

        Ok(stream.map(|try_record| match try_record {
            Ok(record) => Ok((
                record.get(0),
                Self {
                    uid: record.get(1),
                    journals_uid: record.get(2),
                    users_uid: record.get(3),
                    date: record.get(4),
                    title: record.get(5),
                    contents: record.get(6),
                    created: record.get(7),
                    updated: record.get(8),
                    tags: Vec::new(),
                    custom_fields: Vec::new(),
                    files: Vec::new(),
                },
            )),
            Err(err) => Err(error::Error::context_source(
                "failed to retrieve entry record",
                err,
            )),
        }))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryTagSync {
    pub key: String,
    pub value: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl EntryTagSync {
    pub async fn retrieve(
        conn: &impl GenericClient,
        entries_id: &EntryId,
    ) -> Result<Vec<Self>, error::Error> {
        let params: ParamsArray<1> = [entries_id];
        let stream = conn
            .query_raw(
                "\
            select entry_tags.key, \
                   entry_tags.value, \
                   entry_tags.created, \
                   entry_tags.updated \
            from entry_tags \
            where entry_tags.entries_id = $1",
                params,
            )
            .await
            .context("failed to retrieve entry tags")?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record.context("failed to retrieve entry tag record")?;

            rtn.push(Self {
                key: record.get(0),
                value: record.get(1),
                created: record.get(2),
                updated: record.get(3),
            });
        }

        Ok(rtn)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryCFSync {
    pub custom_fields_uid: CustomFieldUid,
    pub value: custom_field::Value,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl EntryCFSync {
    pub async fn retrieve(
        conn: &impl GenericClient,
        entries_id: &EntryId,
    ) -> Result<Vec<Self>, error::Error> {
        let params: ParamsArray<1> = [entries_id];
        let stream = conn
            .query_raw(
                "\
            select custom_fields.uid, \
                   custom_field_entries.value, \
                   custom_field_entries.created, \
                   custom_field_entries.updated \
            from custom_field_entries \
                left join custom_fields on \
                    custom_field_entries.custom_fields_id = custom_fields.id \
            where custom_field_entries.entries_id = $1",
                params,
            )
            .await
            .context("failed to retrieve entry custom fields")?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record.context("failed to retrieve entry custom field record")?;

            rtn.push(Self {
                custom_fields_uid: record.get(0),
                value: record.get(1),
                created: record.get(2),
                updated: record.get(3),
            });
        }

        Ok(rtn)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryFileSync {
    pub uid: FileEntryUid,
    pub name: Option<String>,
    pub mime_type: String,
    pub mime_subtype: String,
    pub mime_param: Option<String>,
    pub size: i64,
    pub hash: Hash,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl EntryFileSync {
    pub async fn retrieve(
        conn: &impl GenericClient,
        entries_id: &EntryId,
    ) -> Result<Vec<Self>, error::Error> {
        let status = FileStatus::Received;
        let params: ParamsArray<2> = [entries_id, &status];
        let stream = conn
            .query_raw(
                "\
            select file_entries.uid, \
                   file_entries.name, \
                   file_entries.mime_type, \
                   file_entries.mime_subtype, \
                   file_entries.mime_param, \
                   file_entries.size, \
                   file_entries.hash, \
                   file_entries.created, \
                   file_entries.updated \
            from file_entries \
            where file_entries.entries_id = $1 and \
                  file_entries.status = $2",
                params,
            )
            .await
            .context("failed to retrieve entry files")?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record.context("failed to retrieve entry file record")?;

            rtn.push(Self {
                uid: record.get(0),
                name: record.get(1),
                mime_type: record.get(2),
                mime_subtype: record.get(3),
                mime_param: record.get(4),
                size: record.get(5),
                hash: record.get(6),
                created: record.get(7),
                updated: record.get(8),
            });
        }

        Ok(rtn)
    }
}
