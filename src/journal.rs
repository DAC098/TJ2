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
    UserId
};

#[derive(Debug, thiserror::Error)]
pub enum JournalCreateError {
    #[error("the given journal name already exists for this user")]
    NameExists,

    #[error("the specified user does not exist")]
    UserNotFound,

    #[error(transparent)]
    Db(#[from] PgError),
}

#[derive(Debug, thiserror::Error)]
pub enum JournalUpdateError {
    #[error("the given journal name already exists for this user")]
    NameExists,

    #[error("the specified journal does not exist")]
    NotFound,

    #[error(transparent)]
    Db(#[from] PgError),
}

#[derive(Debug)]
pub struct JournalCreateOptions {
    users_id: UserId,
    name: String,
    description: Option<String>,
}

impl JournalCreateOptions {
    pub fn description<T>(mut self, value: T) -> Self
    where
        T: Into<String>
    {
        self.description = Some(value.into());
        self
    }
}

#[derive(Debug)]
pub struct Journal {
    pub id: JournalId,
    pub uid: JournalUid,
    pub users_id: UserId,
    pub name: String,
    pub description: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl Journal {
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

#[derive(Debug)]
pub struct Entry {
    pub id: EntryId,
    pub uid: EntryUid,
    pub journals_id: JournalId,
    pub users_id: UserId,
    pub date: NaiveDate,
    pub title: Option<String>,
    pub contents: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl Entry {
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

impl EntryFull {
    pub async fn retrieve_id(
        conn: &impl GenericClient,
        journals_id: &JournalId,
        users_id: &UserId,
        entries_id: &EntryId,
    ) -> Result<Option<Self>, PgError> {
        if let Some(found) = Entry::retrieve_id(conn, journals_id, users_id, entries_id).await ? {
            let tags_fut = EntryTag::retrieve_entry(conn, found.id);
            let files_fut = FileEntry::retrieve_entry(conn, &found.id);

            match tokio::join!(tags_fut, files_fut) {
                (Ok(tags), Ok(files)) => Ok(Some(Self {
                    id: found.id,
                    uid: found.uid,
                    journals_id: found.journals_id,
                    users_id: found.users_id,
                    date: found.date,
                    title: found.title,
                    contents: found.contents,
                    created: found.created,
                    updated: found.updated,
                    tags,
                    files,
                })),
                (Ok(_), Err(err)) => Err(err),
                (Err(err), Ok(_)) => Err(err),
                (Err(tags_err), Err(_files_err)) => Err(tags_err)
            }
        } else {
            Ok(None)
        }
    }
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

    pub async fn retrieve_entry(
        conn: &impl GenericClient,
        entries_id: &EntryId
    ) -> Result<Vec<Self>, PgError> {
        let stream = Self::retrieve_entry_stream(conn, &entries_id).await?;
        let mut rtn = Vec::new();

        futures::pin_mut!(stream);

        while let Some(item) = stream.try_next().await? {
            rtn.push(item);
        }

        Ok(rtn)
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

    pub fn file_path(&self, file_entry: &FileEntry) -> PathBuf {
        self.root.join(format!("files/{}.file", file_entry.id))
    }
}
