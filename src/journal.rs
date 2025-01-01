use std::path::PathBuf;
use std::str::FromStr;

use chrono::{NaiveDate, DateTime, Utc};
use futures::{Stream, StreamExt, TryStream, TryStreamExt};
use serde::Serialize;

use crate::db::{self, GenericClient, PgError};
use crate::db::ids::{
    EntryId,
    EntryUid,
    FileEntryId,
    FileEntryUid,
    JournalId,
    JournalUid,
    UserId,
    CustomFieldId,
    CustomFieldUid,
};

pub mod custom_field;

/// the potential errors when creating a journal
#[derive(Debug, thiserror::Error)]
pub enum JournalCreateError {
    /// the given jounrla name already exists for this user
    #[error("the given journal name already exists for this user")]
    NameExists,

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
}

impl JournalCreateOptions {
    /// assigns a description to the journal
    pub fn description<T>(mut self, value: T) -> Self
    where
        T: Into<String>
    {
        self.description = Some(value.into());
        self
    }
}

/// the database representation of a journal
#[derive(Debug)]
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

impl Journal {
    /// creates the [`JournalCreateOptions`] with the given [`UserId`] and name
    pub fn create_options<N>(users_id: UserId, name: N) -> JournalCreateOptions
    where
        N: Into<String>
    {
        JournalCreateOptions {
            users_id,
            name: name.into(),
            description: None
        }
    }

    /// attempts to create a new [`Journal`] with the given options
    pub async fn create(conn: &impl GenericClient, options: JournalCreateOptions) -> Result<Self, JournalCreateError> {
        let uid = JournalUid::gen();
        let created = Utc::now();
        let users_id = options.users_id;
        let name = options.name;
        let description = options.description;

        let result = conn.query_one(
            "\
            insert into journals (uid, users_id, name, description, created) values \
            ($1, $2, $3, $4, $5) \
            returning id",
            &[
                &uid,
                &users_id,
                &name,
                &description,
                &created
            ]
        ).await;

        match result {
            Ok(row) => Ok(Self {
                id: row.get(0),
                uid,
                users_id,
                name,
                description,
                created,
                updated: None
            }),
            Err(err) => if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::Unique(constraint) => match constraint {
                        "journals_users_id_name_key" => Err(JournalCreateError::NameExists),
                        _ => Err(JournalCreateError::Db(err))
                    }
                    db::ErrorKind::ForeignKey(constraint) => match constraint {
                        "journals_users_id_fkey" => Err(JournalCreateError::UserNotFound),
                        _ => Err(JournalCreateError::Db(err))
                    }
                }
            } else {
                Err(JournalCreateError::Db(err))
            }
        }
    }

    /// attempts to retrieve the journal with the specified [`JournalId`] with
    /// the specified [`UserId`]
    pub async fn retrieve_id(conn: &impl GenericClient, journals_id: &JournalId, users_id: &UserId) -> Result<Option<Self>, PgError> {
        conn.query_opt(
            "\
            select journals.id, \
                   journals.uid, \
                   journals.users_id, \
                   journals.name, \
                   journals.description, \
                   journals.created, \
                   journals.updated \
            from journals \
            where journals.id = $1 and \
                  journals.users_id = $2",
            &[journals_id, users_id]
        )
            .await
            .map(|maybe| maybe.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                users_id: row.get(2),
                name: row.get(3),
                description: row.get(4),
                created: row.get(5),
                updated: row.get(6),
            }))
    }

    /// attempst to update the journal with new data
    ///
    /// only the fields updated, name, and description will be sent to the
    /// database
    pub async fn update(&self, conn: &impl GenericClient) -> Result<(), JournalUpdateError> {
        let result = conn.execute(
            "\
            update journals \
            set updated = $2, \
                name = $3, \
                description = $4 \
            where id = $1",
            &[&self.id, &self.updated, &self.name, &self.description]
        ).await;

        match result {
            Ok(result) => match result {
                1 => Ok(()),
                0 => Err(JournalUpdateError::NotFound),
                _ => unreachable!(),
            }
            Err(err) => if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::Unique(constraint) => match constraint {
                        "journals_users_id_name_key" => Err(JournalUpdateError::NameExists),
                        _ => Err(JournalUpdateError::Db(err)),
                    }
                    // this should not happen as we are not updating foreign
                    // key fields
                    db::ErrorKind::ForeignKey(_) => unreachable!()
                }
            } else {
                Err(JournalUpdateError::Db(err))
            }
        }
    }
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
            &[journals_id, users_id, entries_id]
        )
            .await
            .map(|maybe| maybe.map(|found| Self {
                id: found.get(0),
                uid: found.get(1),
                journals_id: found.get(2),
                users_id: found.get(3),
                date: found.get(4),
                title: found.get(5),
                contents: found.get(6),
                created: found.get(7),
                updated: found.get(8),
            }))
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

impl EntryTag {
    fn map_row(result: Result<tokio_postgres::Row, PgError>) -> Result<Self, PgError> {
        result.map(|record| Self {
            key: record.get(0),
            value: record.get(1),
            created: record.get(2),
            updated: record.get(3),
        })
    }

    pub async fn retrieve_entry_stream(
        conn: &impl GenericClient,
        entry_id: EntryId
    ) -> Result<impl TryStream<Item = Result<Self, PgError>>, PgError> {
        let params: db::ParamsArray<'_, 1> = [&entry_id];

        conn.query_raw(
            "\
            select entry_tags.key, \
                   entry_tags.value, \
                   entry_tags.created, \
                   entry_tags.updated \
            from entry_tags \
            where entry_tags.entries_id = $1",
            params
        )
            .await
            .map(|result| result.map(Self::map_row))
    }

    pub async fn retrieve_entry(conn: &impl GenericClient, entries_id: EntryId) -> Result<Vec<Self>, PgError> {
        let stream = Self::retrieve_entry_stream(conn, entries_id).await?;
        let mut rtn = Vec::new();

        futures::pin_mut!(stream);

        while let Some(item) = stream.try_next().await? {
            rtn.push(item)
        }

        Ok(rtn)
    }
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub id: FileEntryId,
    pub uid: FileEntryUid,
    pub entries_id: EntryId,
    pub name: Option<String>,
    pub mime_type: String,
    pub mime_subtype: String,
    pub mime_param: Option<String>,
    pub size: i64,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl FileEntry {
    pub async fn retrieve_entry_stream(
        conn: &impl GenericClient,
        entries_id: &EntryId
    ) -> Result<impl Stream<Item = Result<Self, PgError>>, PgError> {
        let params: db::ParamsArray<'_, 1> = [entries_id];

        conn.query_raw(
            "\
            select file_entries.id, \
                   file_entries.uid, \
                   file_entries.entries_id, \
                   file_entries.name, \
                   file_entries.mime_type, \
                   file_entries.mime_subtype, \
                   file_entries.mime_param, \
                   file_entries.size, \
                   file_entries.created, \
                   file_entries.updated \
            from file_entries \
            where file_entries.entries_id = $1",
            params
        )
            .await
            .map(|top_res| top_res.map(|stream| stream.map(|record| Self {
                id: record.get(0),
                uid: record.get(1),
                entries_id: record.get(2),
                name: record.get(3),
                mime_type: record.get(4),
                mime_subtype: record.get(5),
                mime_param: record.get(6),
                size: record.get(7),
                created: record.get(8),
                updated: record.get(9),
            })))
    }

    pub async fn retrieve_file_entry(
        conn: &impl GenericClient,
        entries_id: &EntryId,
        file_entry_id: &FileEntryId
    ) -> Result<Option<Self>, PgError> {
        conn.query_opt(
            "\
            select file_entries.id, \
                   file_entries.uid, \
                   file_entries.entries_id, \
                   file_entries.name, \
                   file_entries.mime_type, \
                   file_entries.mime_subtype, \
                   file_entries.mime_param, \
                   file_entries.size, \
                   file_entries.created, \
                   file_entries.updated \
            from file_entries \
            where file_entries.entries_id = $1 and \
                  file_entries.id = $2",
            &[entries_id, file_entry_id]
        )
            .await
            .map(|maybe| maybe.map(|record| Self {
                id: record.get(0),
                uid: record.get(1),
                entries_id: record.get(2),
                name: record.get(3),
                mime_type: record.get(4),
                mime_subtype: record.get(5),
                mime_param: record.get(6),
                size: record.get(7),
                created: record.get(8),
                updated: record.get(9),
            }))
    }

    pub fn get_mime(&self) -> mime::Mime {
        let parse = if let Some(param) = &self.mime_param {
            format!("{}/{};{param}", self.mime_type, self.mime_subtype)
        } else {
            format!("{}/{}", self.mime_type, self.mime_subtype)
        };

        mime::Mime::from_str(&parse)
            .expect("failed to parse MIME from database")
    }

    pub async fn update(&self, conn: &impl GenericClient) -> Result<(), PgError> {
        conn.execute(
            "\
            update file_entries \
            set name = $2, \
                mime_type = $3, \
                mime_subtype = $4, \
                mime_param = $5, \
                size = $6, \
                updated = $7 \
            where file_entries.id = $1",
            &[
                &self.id,
                &self.name,
                &self.mime_type,
                &self.mime_subtype,
                &self.mime_param,
                &self.size,
                &self.updated
            ]
        ).await?;

        Ok(())
    }
}

pub struct CustomFieldOptions {
    journals_id: JournalId,
    name: String,
    pub order: i32,
    pub config: custom_field::Type,
    pub description: Option<String>,
}

impl CustomFieldOptions {
    pub fn new<N>(
        journals_id: JournalId,
        name: N,
        config: custom_field::Type
    ) -> Self
    where
        N: Into<String>
    {
        CustomFieldOptions {
            journals_id,
            name: name.into(),
            order: 0,
            config,
            description: None,
        }
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
    pub async fn create_field(
        conn: &impl GenericClient,
        options: CustomFieldOptions
    ) -> Result<Self, CreateCustomFieldError> {
        let uid = CustomFieldUid::gen();
        let created = Utc::now();
        let CustomFieldOptions {
            journals_id,
            name,
            order,
            config,
            description
        } = options;

        let result = conn.query_one(
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
            &[&uid, &journals_id, &name, &order, &config, &description, &created]
        ).await;

        match result {
            Ok(row) => Ok(Self {
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
            Err(err) => if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::Unique(constraint) => match constraint {
                        "custom_fields_journals_id_name_key" =>
                            Err(CreateCustomFieldError::NameExists),
                        "custom_fields_uid_key" =>
                            Err(CreateCustomFieldError::UidExists),
                        _ => Err(CreateCustomFieldError::Db(err)),
                    }
                    db::ErrorKind::ForeignKey(constraint) => match constraint {
                        "custom_fields_journals_id_fkey" =>
                            Err(CreateCustomFieldError::JournalNotFound),
                        _ => Err(CreateCustomFieldError::Db(err))
                    }
                }
            } else {
                Err(CreateCustomFieldError::Db(err))
            }
        }
    }

    pub async fn retrieve_journal_stream(
        conn: &impl GenericClient,
        journals_id: &JournalId,
    ) -> Result<impl Stream<Item = Result<Self, PgError>>, PgError> {
        let params: db::ParamsArray<'_, 1> = [journals_id];

        Ok(conn.query_raw(
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
            order by custom_fields.\"order\", \
                     custom_fields.name",
            params
        )
            .await?
            .map(|stream| stream.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                journals_id: row.get(2),
                name: row.get(3),
                order: row.get(4),
                config: row.get(5),
                description: row.get(6),
                created: row.get(7),
                updated: row.get(8),
            })))
    }
}

#[derive(Debug)]
pub struct JournalDir {
    root: PathBuf,
}

impl JournalDir {
    pub fn new(root: &PathBuf, journal: &Journal) -> Self {
        let path = format!("journals/{}", journal.id);

        Self {
            root: root.join(path)
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
