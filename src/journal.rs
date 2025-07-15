use std::collections::HashMap;
use std::fmt::Write;
use std::path::PathBuf;
use std::str::FromStr;

use bytes::BytesMut;
use chrono::{DateTime, NaiveDate, Utc};
use futures::{Stream, StreamExt};
use postgres_types as pg_types;
use serde::Serialize;
use serde_repr::Serialize_repr;

use crate::db::ids::{
    CustomFieldId, CustomFieldUid, EntryId, EntryUid, FileEntryId, FileEntryUid, JournalId,
    JournalUid, UserId,
};
use crate::db::{self, GenericClient, PgError};
use crate::error::BoxDynError;
use crate::sec::authn::Initiator;
use crate::sec::authz;
use crate::sec::Hash;

pub mod custom_field;
pub mod sharing;

pub async fn assert_permission(
    conn: &impl db::GenericClient,
    initiator: &Initiator,
    journal: &Journal,
    scope: authz::Scope,
    ability: authz::Ability,
) -> Result<(), authz::PermissionError> {
    if journal.users_id == initiator.user.id {
        tracing::debug!("assert permission");
        authz::assert_permission(conn, initiator.user.id, scope, ability).await
    } else {
        tracing::debug!("assert permission ref");
        authz::assert_permission_ref(conn, initiator.user.id, scope, ability, journal.id).await
    }
}

/// the potential errors when creating a journal
#[derive(Debug, thiserror::Error)]
pub enum JournalCreateError {
    /// the given jounral name already exists for this user
    #[error("the given journal name already exists for this user")]
    NameExists,

    /// the given journal uid already exists
    #[error("the given journal uid already exists")]
    UidExists,

    /// the specified user does not exist
    #[error("the specified user does not exist")]
    UserNotFound,

    #[error(transparent)]
    Db(#[from] PgError),
}

/// the potential errors when updating a journal
#[derive(Debug, thiserror::Error)]
pub enum JournalUpdateError {
    /// the given journal name already exists for this user
    #[error("the given journal name already exists for this user")]
    NameExists,

    /// the specified journal does not exist
    #[error("the specified journal does not exist")]
    NotFound,

    #[error(transparent)]
    Db(#[from] PgError),
}

/// the different optionals available when creating a journal
#[derive(Debug)]
pub struct JournalCreateOptions {
    /// the user to assign the journal to
    users_id: UserId,

    /// the name of the journal
    name: String,

    /// an optional description of the journal
    description: Option<String>,

    /// an optional uid value to assign
    uid: Option<JournalUid>,
}

impl JournalCreateOptions {
    /// assigns a description to the journal
    pub fn description<T>(&mut self, value: T)
    where
        T: Into<String>,
    {
        self.description = Some(value.into());
    }

    pub fn uid(&mut self, value: JournalUid) {
        self.uid = Some(value);
    }
}

#[derive(Debug, Clone)]
pub struct Journal {
    /// the assigned journal id from the database
    pub id: JournalId,

    /// the generated journal uid from the server
    pub uid: JournalUid,

    /// the assigned owner of the journal
    pub users_id: UserId,

    /// the name of the journal
    pub name: String,

    /// the optional description of the journal
    pub description: Option<String>,

    /// timestamp of when the journal was created
    pub created: DateTime<Utc>,

    /// timestamp of when the journal was updated
    pub updated: Option<DateTime<Utc>>,
}

pub enum RetrieveQuery<'a> {
    IdAndUser((&'a JournalId, &'a UserId)),
    Uid(&'a JournalUid),
}

impl<'a> From<(&'a JournalId, &'a UserId)> for RetrieveQuery<'a> {
    fn from(given: (&'a JournalId, &'a UserId)) -> Self {
        Self::IdAndUser(given)
    }
}

impl<'a> From<&'a JournalUid> for RetrieveQuery<'a> {
    fn from(given: &'a JournalUid) -> Self {
        Self::Uid(given)
    }
}

impl Journal {
    /// attempts to retrieve a journal with the given [`RetrieveQuery`]
    pub async fn retrieve<'a, T>(
        conn: &impl GenericClient,
        given: T,
    ) -> Result<Option<Self>, PgError>
    where
        T: Into<RetrieveQuery<'a>>,
    {
        let base = "\
            select journals.id, \
                   journals.uid, \
                   journals.users_id, \
                   journals.name, \
                   journals.description, \
                   journals.created, \
                   journals.updated \
            from journals";

        match given.into() {
            RetrieveQuery::IdAndUser((journals_id, users_id)) => {
                let query = format!(
                    "{base} \
                    where journals.id = $1 and \
                          journals.users_id = $2"
                );

                conn.query_opt(&query, &[journals_id, users_id]).await
            }
            RetrieveQuery::Uid(journals_uid) => {
                let query = format!(
                    "{base} \
                    where journals.uid = $1"
                );

                conn.query_opt(&query, &[journals_uid]).await
            }
        }
        .map(|maybe| {
            maybe.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                users_id: row.get(2),
                name: row.get(3),
                description: row.get(4),
                created: row.get(5),
                updated: row.get(6),
            })
        })
    }

    /// attempts to retrieve the journal with the specified [`JournalId`] with
    /// the specified [`UserId`]
    pub async fn retrieve_id(
        conn: &impl GenericClient,
        journals_id: &JournalId,
        users_id: &UserId,
    ) -> Result<Option<Self>, PgError> {
        Self::retrieve(conn, (journals_id, users_id)).await
    }

    /// creates the [`JournalCreateOptions`] with the given [`UserId`] and name
    pub fn create_options<N>(users_id: UserId, name: N) -> JournalCreateOptions
    where
        N: Into<String>,
    {
        JournalCreateOptions {
            users_id,
            name: name.into(),
            description: None,
            uid: None,
        }
    }

    /// attempts to create a new [`Journal`] with the given options
    pub async fn create(
        conn: &impl GenericClient,
        options: JournalCreateOptions,
    ) -> Result<Self, JournalCreateError> {
        let uid = options.uid.unwrap_or(JournalUid::gen());
        let created = Utc::now();
        let users_id = options.users_id;
        let name = options.name;
        let description = options.description;

        let result = conn
            .query_one(
                "\
            insert into journals (uid, users_id, name, description, created) values \
            ($1, $2, $3, $4, $5) \
            returning id",
                &[&uid, &users_id, &name, &description, &created],
            )
            .await;

        match result {
            Ok(row) => Ok(Self {
                id: row.get(0),
                uid,
                users_id,
                name,
                description,
                created,
                updated: None,
            }),
            Err(err) => {
                if let Some(kind) = db::ErrorKind::check(&err) {
                    match kind {
                        db::ErrorKind::Unique(constraint) => match constraint {
                            "journals_users_id_name_key" => Err(JournalCreateError::NameExists),
                            "journals_uid_key" => Err(JournalCreateError::UidExists),
                            _ => Err(JournalCreateError::Db(err)),
                        },
                        db::ErrorKind::ForeignKey(constraint) => match constraint {
                            "journals_users_id_fkey" => Err(JournalCreateError::UserNotFound),
                            _ => Err(JournalCreateError::Db(err)),
                        },
                    }
                } else {
                    Err(JournalCreateError::Db(err))
                }
            }
        }
    }

    /// attempst to update the journal with new data
    ///
    /// only the fields updated, name, and description will be sent to the
    /// database
    pub async fn update(&self, conn: &impl GenericClient) -> Result<(), JournalUpdateError> {
        let result = conn
            .execute(
                "\
            update journals \
            set updated = $2, \
                name = $3, \
                description = $4 \
            where id = $1",
                &[&self.id, &self.updated, &self.name, &self.description],
            )
            .await;

        match result {
            Ok(result) => match result {
                1 => Ok(()),
                0 => Err(JournalUpdateError::NotFound),
                _ => unreachable!(),
            },
            Err(err) => {
                if let Some(kind) = db::ErrorKind::check(&err) {
                    match kind {
                        db::ErrorKind::Unique(constraint) => match constraint {
                            "journals_users_id_name_key" => Err(JournalUpdateError::NameExists),
                            _ => Err(JournalUpdateError::Db(err)),
                        },
                        // this should not happen as we are not updating foreign
                        // key fields
                        db::ErrorKind::ForeignKey(_) => unreachable!(),
                    }
                } else {
                    Err(JournalUpdateError::Db(err))
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EntryCreateError {
    #[error("the given date entry already exists for this journal")]
    DateExists,

    #[error("the specified journal was not found")]
    JournalNotFound,

    #[error("the specified user was not found")]
    UserNotFound,

    #[error("the specified uid already exists")]
    UidExists,

    #[error(transparent)]
    Db(#[from] db::PgError),
}

#[derive(Debug, thiserror::Error)]
pub enum EntryUpdateError {
    #[error("the given date entry already exists for this journal")]
    DateExists,

    #[error("the given entry was not found")]
    NotFound,

    #[error(transparent)]
    Db(#[from] db::PgError),
}

pub struct EntryCreateOptions {
    journals_id: JournalId,
    users_id: UserId,
    date: NaiveDate,
    pub title: Option<String>,
    pub contents: Option<String>,
}

/// represents an entry in a journal
#[derive(Debug)]
pub struct Entry {
    /// the assigned entry id from the database
    pub id: EntryId,

    /// the generated uid from the server
    pub uid: EntryUid,

    /// the journal that the entry belongs to
    pub journals_id: JournalId,

    /// the user that created the entry
    pub users_id: UserId,

    /// the associated date the entry is for
    pub date: NaiveDate,

    /// an optional title to assign then entry
    pub title: Option<String>,

    /// optional text that can describe anything about the entry
    pub contents: Option<String>,

    /// timestamp of when the entry was created
    pub created: DateTime<Utc>,

    /// timestamp of when the entry was updated
    pub updated: Option<DateTime<Utc>>,
}

impl Entry {
    pub fn create_options(
        journals_id: JournalId,
        users_id: UserId,
        date: NaiveDate,
    ) -> EntryCreateOptions {
        EntryCreateOptions {
            journals_id,
            users_id,
            date,
            title: None,
            contents: None,
        }
    }

    pub async fn create(
        conn: &impl GenericClient,
        options: EntryCreateOptions,
    ) -> Result<Self, EntryCreateError> {
        let uid = EntryUid::gen();
        let created = Utc::now();
        let EntryCreateOptions {
            journals_id,
            users_id,
            date,
            title,
            contents,
        } = options;

        let result = conn
            .query_one(
                "\
            insert into entries (uid, journals_id, users_id, entry_date, title, contents, created) \
            values ($1, $2, $3, $4, $5, $6, $7) \
            returning id",
                &[
                    &uid,
                    &journals_id,
                    &users_id,
                    &date,
                    &title,
                    &contents,
                    &created,
                ],
            )
            .await;

        match result {
            Ok(row) => Ok(Self {
                id: row.get(0),
                uid,
                journals_id,
                users_id,
                date,
                title,
                contents,
                created,
                updated: None,
            }),
            Err(err) => {
                if let Some(kind) = db::ErrorKind::check(&err) {
                    match kind {
                        db::ErrorKind::Unique(constraint) => match constraint {
                            "entries_journals_id_entry_date_key" => {
                                Err(EntryCreateError::DateExists)
                            }
                            "entries_uid_key" => Err(EntryCreateError::UidExists),
                            _ => Err(EntryCreateError::Db(err)),
                        },
                        db::ErrorKind::ForeignKey(constraint) => match constraint {
                            "entries_journals_id_fkey" => Err(EntryCreateError::JournalNotFound),
                            "entries_users_id_fkey" => Err(EntryCreateError::UserNotFound),
                            _ => Err(EntryCreateError::Db(err)),
                        },
                    }
                } else {
                    Err(EntryCreateError::Db(err))
                }
            }
        }
    }

    /// attempts to retrieve the specified entry for the [`JournalId`],
    /// [`UserId`], and [`EntryId`]
    pub async fn retrieve_id(
        conn: &impl GenericClient,
        journals_id: &JournalId,
        users_id: &UserId,
        entries_id: &EntryId,
    ) -> Result<Option<Self>, PgError> {
        conn.query_opt(
            "\
            select entries.id, \
                   entries.uid, \
                   entries.journals_id, \
                   entries.users_id, \
                   entries.entry_date, \
                   entries.title, \
                   entries.contents, \
                   entries.created, \
                   entries.updated \
            from entries \
            where entries.journals_id = $1 and \
                  entries.id = $3 and \
                  entries.users_id = $2",
            &[journals_id, users_id, entries_id],
        )
        .await
        .map(|maybe| {
            maybe.map(|found| Self {
                id: found.get(0),
                uid: found.get(1),
                journals_id: found.get(2),
                users_id: found.get(3),
                date: found.get(4),
                title: found.get(5),
                contents: found.get(6),
                created: found.get(7),
                updated: found.get(8),
            })
        })
    }

    pub async fn update(&mut self, conn: &impl GenericClient) -> Result<(), EntryUpdateError> {
        let result = conn
            .execute(
                "\
            update entries \
            set entry_date = $2, \
                title = $3, \
                contents = $4, \
                updated = $5 \
            where id = $1",
                &[
                    &self.id,
                    &self.date,
                    &self.title,
                    &self.contents,
                    &self.updated,
                ],
            )
            .await;

        match result {
            Ok(count) => match count {
                1 => Ok(()),
                0 => Err(EntryUpdateError::NotFound),
                _ => unreachable!(),
            },
            Err(err) => {
                if let Some(kind) = db::ErrorKind::check(&err) {
                    match kind {
                        db::ErrorKind::Unique(constraint) => match constraint {
                            "entries_journals_id_entry_date_key" => {
                                Err(EntryUpdateError::DateExists)
                            }
                            _ => Err(EntryUpdateError::Db(err)),
                        },
                        _ => Err(EntryUpdateError::Db(err)),
                    }
                } else {
                    Err(EntryUpdateError::Db(err))
                }
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EntryFull<Files = FileEntry>
where
    Files: Serialize,
{
    pub id: EntryId,
    pub uid: EntryUid,
    pub journals_id: JournalId,
    pub users_id: UserId,
    pub date: NaiveDate,
    pub title: Option<String>,
    pub contents: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
    pub tags: Vec<EntryTag>,
    pub files: Vec<Files>,
}

#[derive(Debug, Serialize)]
pub struct EntryTag {
    pub key: String,
    pub value: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize_repr)]
#[repr(i16)]
pub enum FileStatus {
    Requested = 0,
    Received = 1,
}

#[derive(Debug, thiserror::Error)]
#[error("the given value is an invalid FileStatus")]
pub struct InvalidFileStatus;

impl TryFrom<i16> for FileStatus {
    type Error = InvalidFileStatus;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Requested),
            1 => Ok(Self::Received),
            _ => Err(InvalidFileStatus),
        }
    }
}

impl pg_types::ToSql for FileStatus {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
        (*self as i16).to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i16 as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl<'a> pg_types::FromSql<'a> for FileStatus {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let value = <i16 as pg_types::FromSql>::from_sql(ty, raw)?;

        match value.try_into() {
            Ok(rep) => Ok(rep),
            Err(_err) => {
                Err("invalid sql value for FileStatus. expected smallint with valid status".into())
            }
        }
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i16 as pg_types::FromSql>::accepts(ty)
    }
}

#[derive(Debug, Serialize)]
pub enum FileEntry {
    Requested(RequestedFile),
    Received(ReceivedFile),
}

#[derive(Debug, Serialize)]
pub struct RequestedFile {
    pub id: FileEntryId,
    pub uid: FileEntryUid,
    pub entries_id: EntryId,
    pub name: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct RequestedFileBuilder {
    entries_id: EntryId,
    name: Option<String>,
    uid: Option<FileEntryUid>,
    created: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ReceivedFile {
    pub id: FileEntryId,
    pub uid: FileEntryUid,
    pub entries_id: EntryId,
    pub name: Option<String>,
    pub mime_type: String,
    pub mime_subtype: String,
    pub mime_param: Option<String>,
    pub size: i64,
    pub hash: Hash,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

pub enum RetrieveFileEntryQuery<'a> {
    EntryAndId((&'a EntryId, &'a FileEntryId)),
    Uid(&'a FileEntryUid),
}

impl<'a> From<(&'a EntryId, &'a FileEntryId)> for RetrieveFileEntryQuery<'a> {
    fn from(given: (&'a EntryId, &'a FileEntryId)) -> Self {
        Self::EntryAndId(given)
    }
}

impl<'a> From<&'a FileEntryUid> for RetrieveFileEntryQuery<'a> {
    fn from(given: &'a FileEntryUid) -> Self {
        Self::Uid(given)
    }
}

impl FileEntry {
    pub async fn retrieve_entry_stream(
        conn: &impl GenericClient,
        entries_id: &EntryId,
    ) -> Result<impl Stream<Item = Result<Self, PgError>>, PgError> {
        let params: db::ParamsArray<'_, 1> = [entries_id];

        Ok(conn
            .query_raw(
                "\
            select file_entries.id, \
                   file_entries.uid, \
                   file_entries.entries_id, \
                   file_entries.status, \
                   file_entries.name, \
                   file_entries.mime_type, \
                   file_entries.mime_subtype, \
                   file_entries.mime_param, \
                   file_entries.size, \
                   file_entries.hash, \
                   file_entries.created, \
                   file_entries.updated \
            from file_entries \
            where file_entries.entries_id = $1",
                params,
            )
            .await?
            .map(|result| {
                result.map(|record| match record.get::<usize, FileStatus>(3) {
                    FileStatus::Requested => Self::Requested(RequestedFile {
                        id: record.get(0),
                        uid: record.get(1),
                        entries_id: record.get(2),
                        name: record.get(4),
                        created: record.get(10),
                        updated: record.get(11),
                    }),
                    FileStatus::Received => Self::Received(ReceivedFile {
                        id: record.get(0),
                        uid: record.get(1),
                        entries_id: record.get(2),
                        name: record.get(4),
                        mime_type: record.get(5),
                        mime_subtype: record.get(6),
                        mime_param: record.get(7),
                        size: record.get(8),
                        hash: record.get(9),
                        created: record.get(10),
                        updated: record.get(11),
                    }),
                })
            }))
    }

    pub async fn retrieve<'a, T>(
        conn: &impl GenericClient,
        given: T,
    ) -> Result<Option<Self>, PgError>
    where
        T: Into<RetrieveFileEntryQuery<'a>>,
    {
        let base = "\
            select file_entries.id, \
                   file_entries.uid, \
                   file_entries.entries_id, \
                   file_entries.status, \
                   file_entries.name, \
                   file_entries.mime_type, \
                   file_entries.mime_subtype, \
                   file_entries.mime_param, \
                   file_entries.size, \
                   file_entries.hash, \
                   file_entries.created, \
                   file_entries.updated \
            from file_entries";

        let result = match given.into() {
            RetrieveFileEntryQuery::EntryAndId((entries_id, file_entry_id)) => {
                let query = format!(
                    "{base} \
                    where file_entries.entries_id = $1 and \
                          file_entries.id = $2"
                );

                conn.query_opt(&query, &[entries_id, file_entry_id]).await?
            }
            RetrieveFileEntryQuery::Uid(uid) => {
                let query = format!("{base} where file_entries.uid = $1");

                conn.query_opt(&query, &[uid]).await?
            }
        };

        Ok(
            result.map(|record| match record.get::<usize, FileStatus>(3) {
                FileStatus::Requested => Self::Requested(RequestedFile {
                    id: record.get(0),
                    uid: record.get(1),
                    entries_id: record.get(2),
                    name: record.get(4),
                    created: record.get(10),
                    updated: record.get(11),
                }),
                FileStatus::Received => Self::Received(ReceivedFile {
                    id: record.get(0),
                    uid: record.get(1),
                    entries_id: record.get(2),
                    name: record.get(4),
                    mime_type: record.get(5),
                    mime_subtype: record.get(6),
                    mime_param: record.get(7),
                    size: record.get(8),
                    hash: record.get(9),
                    created: record.get(10),
                    updated: record.get(11),
                }),
            }),
        )
    }

    pub async fn retrieve_file_entry(
        conn: &impl GenericClient,
        entries_id: &EntryId,
        file_entry_id: &FileEntryId,
    ) -> Result<Option<Self>, PgError> {
        Self::retrieve(conn, (entries_id, file_entry_id)).await
    }

    pub async fn retrieve_uid_map(
        conn: &impl GenericClient,
        entries_id: &EntryId,
    ) -> Result<HashMap<FileEntryUid, Self>, PgError> {
        let stream = Self::retrieve_entry_stream(conn, entries_id).await?;

        futures::pin_mut!(stream);

        let mut rtn = HashMap::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record?;

            rtn.insert(record.uid().clone(), record);
        }

        Ok(rtn)
    }

    pub fn id_ref(&self) -> &FileEntryId {
        match self {
            Self::Requested(req) => &req.id,
            Self::Received(rec) => &rec.id,
        }
    }

    pub fn uid(&self) -> &FileEntryUid {
        match self {
            Self::Requested(req) => &req.uid,
            Self::Received(rec) => &rec.uid,
        }
    }

    pub fn entries_id(&self) -> EntryId {
        match self {
            Self::Requested(req) => req.entries_id,
            Self::Received(rec) => rec.entries_id,
        }
    }

    pub fn into_received(self) -> Result<ReceivedFile, Self> {
        match self {
            Self::Received(rec) => Ok(rec),
            _ => Err(self),
        }
    }

    pub fn into_requested(self) -> Result<RequestedFile, Self> {
        match self {
            Self::Requested(req) => Ok(req),
            _ => Err(self),
        }
    }
}

pub struct PromoteOptions {
    pub mime: mime::Mime,
    pub size: i64,
    pub hash: Hash,
    pub created: DateTime<Utc>,
}

impl RequestedFileBuilder {
    pub fn with_name<T>(&mut self, name: T)
    where
        T: Into<String>,
    {
        self.name = Some(name.into());
    }

    pub fn with_uid(&mut self, uid: FileEntryUid) {
        self.uid = Some(uid);
    }

    pub fn with_created(&mut self, created: DateTime<Utc>) {
        self.created = Some(created);
    }

    pub async fn build_many(
        conn: &impl db::GenericClient,
        mut files: Vec<Self>,
    ) -> Result<Option<impl Stream<Item = Result<RequestedFile, PgError>>>, PgError> {
        if files.is_empty() {
            return Ok(None);
        }

        let status = FileStatus::Requested;
        let created = Utc::now();
        let mut params: db::ParamsVec<'_> = vec![&status];
        let mut query = String::from(
            "\
            insert into file_entries ( \
                uid, \
                entries_id, \
                status, \
                name, \
                created, \
                mime_type, \
                mime_subtype, \
                hash \
            ) values ",
        );

        for (index, file) in files.iter_mut().enumerate() {
            if file.uid.is_none() {
                file.uid = Some(FileEntryUid::gen());
            }

            if file.created.is_none() {
                file.created = Some(created);
            }

            if index != 0 {
                query.push_str(", ");
            }

            write!(
                &mut query,
                "(${}, ${}, $1, ${}, ${}, '', '', '')",
                db::push_param(&mut params, &file.uid),
                db::push_param(&mut params, &file.entries_id),
                db::push_param(&mut params, &file.name),
                db::push_param(&mut params, &file.created),
            )
            .unwrap();
        }

        query.push_str(" returning id");

        Ok(Some(
            conn.query_raw(&query, params)
                .await?
                .zip(futures::stream::iter(files))
                .map(
                    |(
                        result_row,
                        Self {
                            uid,
                            entries_id,
                            name,
                            created,
                        },
                    )| {
                        result_row.map(|row| RequestedFile {
                            id: row.get(0),
                            uid: uid.unwrap(),
                            entries_id,
                            name,
                            created: created.unwrap(),
                            updated: None,
                        })
                    },
                ),
        ))
    }
}

impl RequestedFile {
    pub fn builder(entries_id: EntryId) -> RequestedFileBuilder {
        RequestedFileBuilder {
            entries_id,
            name: None,
            uid: None,
            created: None,
        }
    }

    pub async fn promote(
        self,
        conn: &impl GenericClient,
        PromoteOptions {
            mime,
            size,
            hash,
            created,
        }: PromoteOptions,
    ) -> Result<ReceivedFile, (Self, PgError)> {
        let status = FileStatus::Received;
        let mime_type = mime.type_().as_str().to_owned();
        let mime_subtype = mime.subtype().as_str().to_owned();
        let mime_param = get_mime_param(mime.params());

        let result = conn
            .execute(
                "\
            update file_entries \
            set name = $2, \
                mime_type = $3, \
                mime_subtype = $4, \
                mime_param = $5, \
                size = $6, \
                hash = $7, \
                created = $8, \
                status = $9 \
            where id = $1",
                &[
                    &self.id,
                    &self.name,
                    &mime_type,
                    &mime_subtype,
                    &mime_param,
                    &size,
                    &hash,
                    &created,
                    &status,
                ],
            )
            .await;

        match result {
            Ok(_) => Ok(ReceivedFile {
                id: self.id,
                uid: self.uid,
                entries_id: self.entries_id,
                name: self.name,
                mime_type,
                mime_subtype,
                mime_param,
                size,
                hash,
                created,
                updated: None,
            }),
            Err(err) => Err((self, err)),
        }
    }
}

impl ReceivedFile {
    pub fn get_mime(&self) -> mime::Mime {
        let parse = if let Some(param) = &self.mime_param {
            format!("{}/{};{param}", self.mime_type, self.mime_subtype)
        } else {
            format!("{}/{}", self.mime_type, self.mime_subtype)
        };

        mime::Mime::from_str(&parse).expect("failed to parse MIME from database")
    }
}

fn get_mime_param(params: mime::Params<'_>) -> Option<String> {
    let collected = params
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<String>>()
        .join(";");

    if !collected.is_empty() {
        Some(collected)
    } else {
        None
    }
}

pub struct CustomFieldBuilder {
    journals_id: JournalId,
    name: String,
    order: i32,
    config: custom_field::Type,
    description: Option<String>,
    uid: Option<CustomFieldUid>,
    created: Option<DateTime<Utc>>,
}

impl CustomFieldBuilder {
    pub fn with_order(&mut self, order: i32) {
        self.order = order;
    }

    pub fn with_description<T>(&mut self, description: T)
    where
        T: Into<String>,
    {
        self.description = Some(description.into());
    }

    pub fn with_uid(&mut self, uid: CustomFieldUid) {
        self.uid = Some(uid);
    }

    pub fn with_created(&mut self, created: DateTime<Utc>) {
        self.created = Some(created);
    }

    pub async fn build(
        self,
        conn: &impl db::GenericClient,
    ) -> Result<CustomField, CreateCustomFieldError> {
        let Self {
            journals_id,
            name,
            order,
            config,
            description,
            uid,
            created,
        } = self;
        let uid = uid.unwrap_or(CustomFieldUid::gen());
        let created = created.unwrap_or(Utc::now());

        let result = conn
            .query_one(
                "\
            insert into custom_fields (\
                uid, \
                journals_id, \
                name, \
                \"order\", \
                config, \
                description, \
                created \
            ) values ($1, $2, $3, $4, $5, $6, $7) \
            returning id",
                &[
                    &uid,
                    &journals_id,
                    &name,
                    &order,
                    &config,
                    &description,
                    &created,
                ],
            )
            .await;

        match result {
            Ok(row) => Ok(CustomField {
                id: row.get(0),
                uid,
                journals_id,
                name,
                order,
                config,
                description,
                created,
                updated: None,
            }),
            Err(err) => {
                if let Some(kind) = db::ErrorKind::check(&err) {
                    match kind {
                        db::ErrorKind::Unique(constraint) => match constraint {
                            "custom_fields_journals_id_name_key" => {
                                Err(CreateCustomFieldError::NameExists)
                            }
                            "custom_fields_uid_key" => Err(CreateCustomFieldError::UidExists),
                            _ => Err(CreateCustomFieldError::Db(err)),
                        },
                        db::ErrorKind::ForeignKey(constraint) => match constraint {
                            "custom_fields_journals_id_fkey" => {
                                Err(CreateCustomFieldError::JournalNotFound)
                            }
                            _ => Err(CreateCustomFieldError::Db(err)),
                        },
                    }
                } else {
                    Err(CreateCustomFieldError::Db(err))
                }
            }
        }
    }

    pub async fn build_many(
        conn: &impl db::GenericClient,
        mut cfs: Vec<Self>,
    ) -> Result<Option<impl Stream<Item = Result<CustomField, PgError>>>, PgError> {
        if cfs.is_empty() {
            return Ok(None);
        }

        let mut query = String::from(
            "insert into custom_fields (uid, journals_id, name, \"order\", config, description, created) values"
        );
        let mut params: db::ParamsVec<'_> = Vec::new();
        let created = Utc::now();

        for (index, field) in cfs.iter_mut().enumerate() {
            if field.uid.is_none() {
                field.uid = Some(CustomFieldUid::gen());
            }

            if field.created.is_none() {
                field.created = Some(created);
            }

            if index > 0 {
                query.push_str(", ");
            }

            let s = format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${})",
                db::push_param(&mut params, &field.uid),
                db::push_param(&mut params, &field.journals_id),
                db::push_param(&mut params, &field.name),
                db::push_param(&mut params, &field.order),
                db::push_param(&mut params, &field.config),
                db::push_param(&mut params, &field.description),
                db::push_param(&mut params, &field.created),
            );

            query.push_str(&s);
        }

        query.push_str(" returning id");

        Ok(Some(
            conn.query_raw(&query, params)
                .await?
                .zip(futures::stream::iter(cfs))
                .map(
                    |(
                        result_row,
                        Self {
                            uid,
                            journals_id,
                            name,
                            order,
                            config,
                            description,
                            created,
                        },
                    )| {
                        result_row.map(|row| CustomField {
                            id: row.get(0),
                            uid: uid.unwrap(),
                            journals_id,
                            name,
                            order,
                            config,
                            description,
                            created: created.unwrap(),
                            updated: None,
                        })
                    },
                ),
        ))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateCustomFieldError {
    #[error("the given custom field uid already exists")]
    UidExists,

    #[error("the given name already exists for this journal")]
    NameExists,

    #[error("the specified journal does not exist")]
    JournalNotFound,

    #[error(transparent)]
    Db(#[from] PgError),
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateCustomFieldError {
    #[error("the specified custom field was not found")]
    NotFound,

    #[error("the given name already exists for this journal")]
    NameExists,

    #[error(transparent)]
    Db(#[from] db::PgError),
}

#[derive(Debug)]
pub struct CustomField {
    pub id: CustomFieldId,
    pub uid: CustomFieldUid,
    pub journals_id: JournalId,
    pub name: String,
    pub order: i32,
    pub config: custom_field::Type,
    pub description: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl CustomField {
    pub fn builder<N>(
        journals_id: JournalId,
        name: N,
        config: custom_field::Type,
    ) -> CustomFieldBuilder
    where
        N: Into<String>,
    {
        CustomFieldBuilder {
            journals_id,
            name: name.into(),
            order: 0,
            config,
            description: None,
            uid: None,
            created: None,
        }
    }

    pub async fn retrieve_journal_stream(
        conn: &impl GenericClient,
        journals_id: &JournalId,
    ) -> Result<impl Stream<Item = Result<Self, PgError>>, PgError> {
        let params: db::ParamsArray<'_, 1> = [journals_id];

        Ok(conn
            .query_raw(
                "\
            select custom_fields.id, \
                   custom_fields.uid, \
                   custom_fields.journals_id, \
                   custom_fields.name, \
                   custom_fields.\"order\", \
                   custom_fields.config, \
                   custom_fields.description, \
                   custom_fields.created, \
                   custom_fields.updated \
            from custom_fields \
            where custom_fields.journals_id = $1 \
            order by custom_fields.\"order\" desc, \
                     custom_fields.name",
                params,
            )
            .await?
            .map(|stream| {
                stream.map(|row| Self {
                    id: row.get(0),
                    uid: row.get(1),
                    journals_id: row.get(2),
                    name: row.get(3),
                    order: row.get(4),
                    config: row.get(5),
                    description: row.get(6),
                    created: row.get(7),
                    updated: row.get(8),
                })
            }))
    }

    pub async fn retrieve_journal_map(
        conn: &impl GenericClient,
        journals_id: &JournalId,
    ) -> Result<HashMap<CustomFieldId, Self>, PgError> {
        let stream = Self::retrieve_journal_stream(conn, journals_id).await?;

        futures::pin_mut!(stream);

        let mut rtn = HashMap::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record?;

            rtn.insert(record.id, record);
        }

        Ok(rtn)
    }

    pub async fn retrieve_journal_uid_map(
        conn: &impl GenericClient,
        journals_id: &JournalId,
    ) -> Result<HashMap<CustomFieldUid, Self>, PgError> {
        let stream = Self::retrieve_journal_stream(conn, journals_id).await?;

        futures::pin_mut!(stream);

        let mut rtn = HashMap::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record?;

            rtn.insert(record.uid.clone(), record);
        }

        Ok(rtn)
    }

    pub async fn update(
        &self,
        conn: &impl db::GenericClient,
    ) -> Result<(), UpdateCustomFieldError> {
        let params: db::ParamsArray<'_, 5> = [
            &self.id,
            &self.name,
            &self.order,
            &self.description,
            &self.updated,
        ];

        let result = conn
            .execute(
                "\
            update custom_fields \
            set name = $2, \
                \"order\" = $3, \
                description = $4, \
                updated = $5 \
            where id = $1",
                &params,
            )
            .await;

        match result {
            Ok(executed) => match executed {
                1 => Ok(()),
                0 => Err(UpdateCustomFieldError::NotFound),
                _ => unreachable!(),
            },
            Err(err) => {
                if let Some(kind) = db::ErrorKind::check(&err) {
                    match kind {
                        db::ErrorKind::Unique(constraint) => match constraint {
                            "custom_fields_journals_id_name_key" => {
                                Err(UpdateCustomFieldError::NameExists)
                            }
                            _ => Err(UpdateCustomFieldError::Db(err)),
                        },
                        _ => Err(UpdateCustomFieldError::Db(err)),
                    }
                } else {
                    Err(UpdateCustomFieldError::Db(err))
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct JournalDir {
    root: PathBuf,
}

impl JournalDir {
    pub fn new(root: &PathBuf, journals_id: JournalId) -> Self {
        let path = format!("journals/{journals_id}");

        Self {
            root: root.join(path),
        }
    }

    pub async fn create_root_dir(&self) -> Result<PathBuf, std::io::Error> {
        tokio::fs::create_dir(&self.root).await?;

        Ok(self.root.clone())
    }

    pub async fn create_files_dir(&self) -> Result<PathBuf, std::io::Error> {
        let files_dir = self.root.join("files");

        tokio::fs::create_dir(&files_dir).await?;

        Ok(files_dir)
    }

    pub async fn create(&self) -> Result<(), std::io::Error> {
        self.create_root_dir().await?;
        self.create_files_dir().await?;

        Ok(())
    }

    pub fn file_path(&self, file_entries_id: &FileEntryId) -> PathBuf {
        self.root.join(format!("files/{}.file", file_entries_id))
    }
}
