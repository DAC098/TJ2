use bytes::BytesMut;
use chrono::{NaiveDate, DateTime, Utc};
use postgres_types as pg_types;
use serde::{Serialize, Deserialize};

use crate::db::ids::{
    JournalUid,
    EntryUid,
    CustomFieldUid,
    FileEntryUid,
    UserUid,
};
use crate::error::BoxDynError;
use crate::journal::custom_field;

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
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        let v: i16 = self.into();

        v.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i16 as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
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

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryTagSync {
    pub key: String,
    pub value: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryCFSync {
    pub custom_fields_uid: CustomFieldUid,
    pub value: custom_field::Value,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryFileSync {
    pub uid: FileEntryUid,
    pub name: Option<String>,
    pub mime_type: String,
    pub mime_subtype: String,
    pub mime_param: Option<String>,
    pub size: i64,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}
