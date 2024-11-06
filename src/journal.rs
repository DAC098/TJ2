use std::str::FromStr;

use chrono::{NaiveDate, DateTime, Utc};
use futures::{Stream, StreamExt, TryStream, TryStreamExt, FutureExt};
use serde::Serialize;
use sqlx::Row;

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

#[derive(Debug)]
pub struct Journal {
    pub id: JournalId,
    pub uid: JournalUid,
    pub users_id: UserId,
    pub name: String,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl Journal {
    pub async fn create_pg(conn: &impl GenericClient, users_id: UserId, name: &str) -> Result<Self, PgError> {
        let uid = JournalUid::gen();
        let created = Utc::now();

        conn.query_one(
            "\
            insert into journals (uid, users_id, name, created) values \
            ($1, $2, $3, $4) \
            returning id",
            &[&uid, &users_id, &name, &created]
        )
            .await
            .map(|row| Self {
                id: row.get(0),
                uid,
                users_id,
                name: name.to_owned(),
                created,
                updated: None
            })
    }

    pub async fn create(conn: &mut db::DbConn, users_id: UserId, name: &str) -> Result<Self, sqlx::Error> {
        let uid = JournalUid::gen();
        let created = Utc::now();

        sqlx::query(
            "\
            insert into journals (uid, users_id, name, created) values \
            (?1, ?2, ?3, ?4) \
            returning id"
        )
            .bind(&uid)
            .bind(users_id)
            .bind(name)
            .bind(created)
            .fetch_one(conn)
            .await
            .map(|row| Self {
                id: row.get(0),
                uid,
                users_id,
                name: name.to_owned(),
                created,
                updated: None
            })
    }

    pub async fn retrieve_default_pg(conn: &impl GenericClient, users_id: UserId) -> Result<Option<Self>, PgError> {
        conn.query_opt(
            "\
            select journals.id, \
                   journals.uid, \
                   journals.users_id, \
                   journals.name, \
                   journals.created, \
                   journals.updated \
            from journals \
            where journals.name = 'default' and \
                  journals.users_id = $1",
            &[&users_id]
        )
            .await
            .map(|maybe| maybe.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                users_id: row.get(2),
                name: row.get(3),
                created: row.get(4),
                updated: row.get(5),
            }))
    }

    pub async fn retrieve_default(conn: &mut db::DbConn, users_id: UserId) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query(
            "\
            select journals.id, \
                   journals.uid, \
                   journals.users_id, \
                   journals.name, \
                   journals.created, \
                   journals.updated \
            from journals \
            where journals.name = 'default'"
        )
            .bind(users_id)
            .fetch_optional(conn)
            .await
            .map(|maybe| maybe.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                users_id: row.get(2),
                name: row.get(3),
                created: row.get(4),
                updated: row.get(5),
            }))
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
    pub async fn retrieve_date_pg(
        conn: &impl GenericClient,
        journals_id: JournalId,
        users_id: UserId,
        date: &NaiveDate
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
                  entries.entry_date = $2 and \
                  entries.users_id = $3",
            &[&journals_id, &users_id, date]
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

    pub async fn retrieve_date(
        conn: &mut db::DbConn,
        journals_id: JournalId,
        users_id: UserId,
        date: &NaiveDate
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query(
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
            where entries.journals_id = ?1 and \
                  entries.entry_date = ?2 and \
                  entries.users_id = ?3"
        )
            .bind(journals_id)
            .bind(date)
            .bind(users_id)
            .fetch_optional(&mut *conn)
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
    pub async fn retrieve_date_pg(
        conn: &impl GenericClient,
        journals_id: JournalId,
        users_id: UserId,
        date: &NaiveDate,
    ) -> Result<Option<Self>, PgError> {
        if let Some(found) = Entry::retrieve_date_pg(conn, journals_id, users_id, date).await ? {
            let tags_fut = EntryTag::retrieve_entry_pg(conn, found.id);
            let files_fut = FileEntry::retrieve_entry_pg(conn, found.id);

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

    pub async fn retrieve_date(
        conn: &mut db::DbConn,
        journals_id: JournalId,
        users_id: UserId,
        date: &NaiveDate
    ) -> Result<Option<Self>, sqlx::Error> {
        if let Some(found) = Entry::retrieve_date(conn, journals_id, users_id, date).await? {
            let tags = EntryTag::retrieve_date(conn, users_id, date)
                .await?;
            let files = FileEntry::retrieve_entry(conn, found.id)
                .await?;

            Ok(Some(Self {
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
            }))
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

    pub async fn retrieve_entry_stream_pg(
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

    pub fn retrieve_entry_stream(
        conn: &mut db::DbConn,
        entry_id: EntryId
    ) -> impl Stream<Item = Result<Self, sqlx::Error>> + '_ {
        sqlx::query(
            "\
            select entry_tags.key, \
                   entry_tags.value, \
                   entry_tags.created, \
                   entry_tags.updated \
            from entry_tags \
            where entry_tags.entries_id = ?1"
        )
            .bind(entry_id)
            .fetch(&mut *conn)
            .map(|res| res.map(|record| Self {
                key: record.get(0),
                value: record.get(1),
                created: record.get(2),
                updated: record.get(3),
            }))
    }

    pub async fn retrieve_date_stream_pg(
        conn: &impl GenericClient,
        users_id: &UserId,
        date: &NaiveDate,
    ) -> Result<impl TryStream<Item = Result<Self, PgError>>, PgError> {
        let params: db::ParamsArray<'_, 2> = [users_id, date];

        conn.query_raw(
            "\
            select entry_tags.key, \
                   entry_tags.value, \
                   entry_tags.created, \
                   entry_tags.updated \
            from entry_tags \
                left join entries on \
                    entry_tags.entries_id = entries.id \
            where entries.entry_date = $1 and \
                  entries.users_id = $2",
            params
        )
            .await
            .map(|result| result.map(Self::map_row))
    }

    pub fn retrieve_date_stream<'a>(
        conn: &'a mut db::DbConn,
        users_id: UserId,
        date: &'a NaiveDate
    ) -> impl Stream<Item = Result<Self, sqlx::Error>> + 'a {
        sqlx::query(
            "\
            select entry_tags.key, \
                   entry_tags.value, \
                   entry_tags.created, \
                   entry_tags.updated \
            from entry_tags \
                left join entries on \
                    entry_tags.entries_id = entries.id \
            where entries.entry_date = ?1 and \
                  entries.users_id = ?2"
        )
            .bind(date)
            .bind(users_id)
            .fetch(&mut *conn)
            .map(|res| res.map(|record| Self {
                key: record.get(0),
                value: record.get(1),
                created: record.get(2),
                updated: record.get(3),
            }))
    }

    pub async fn retrieve_entry_pg(conn: &impl GenericClient, entries_id: EntryId) -> Result<Vec<Self>, PgError> {
        let stream = Self::retrieve_entry_stream_pg(conn, entries_id).await?;
        let mut rtn = Vec::new();

        futures::pin_mut!(stream);

        while let Some(item) = stream.try_next().await? {
            rtn.push(item)
        }

        Ok(rtn)
    }

    pub async fn retrieve_date_pg(conn: &impl GenericClient, users_id: UserId, date: &NaiveDate) -> Result<Vec<Self>, PgError> {
        let stream = Self::retrieve_date_stream_pg(conn, &users_id, date).await?;
        let mut rtn = Vec::new();

        futures::pin_mut!(stream);

        while let Some(item) = stream.try_next().await? {
            rtn.push(item);
        }

        Ok(rtn)
    }

    pub async fn retrieve_date(conn: &mut db::DbConn, users_id: UserId, date: &NaiveDate) -> Result<Vec<Self>, sqlx::Error> {
        Self::retrieve_date_stream(conn, users_id, date)
            .try_collect()
            .await
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
    pub async fn retrieve_entry_stream_pg(
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
                   file_entries.mime_parameter, \
                   file_entries.size, \
                   file_entries.created, \
                   file_entries.updated \
            from file_entries \
            where file_entries.entries_id = ?1",
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

    pub fn retrieve_entry_stream(
        conn: &mut db::DbConn,
        entries_id: EntryId
    ) -> impl Stream<Item = Result<Self, sqlx::Error>> + '_ {
        sqlx::query(
            "\
            select file_entries.id, \
                   file_entries.uid, \
                   file_entries.entries_id, \
                   file_entries.name, \
                   file_entries.mime_type, \
                   file_entries.mime_subtype, \
                   file_entries.mime_parameter, \
                   file_entries.size, \
                   file_entries.created, \
                   file_entries.updated \
            from file_entries \
            where file_entries.entries_id = ?1"
        )
            .bind(entries_id)
            .fetch(&mut *conn)
            .map(|res| res.map(|record| Self {
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

    pub async fn retrieve_file_entry_pg(
        conn: &impl GenericClient,
        date: &NaiveDate,
        file_entry_id: FileEntryId
    ) -> Result<Option<Self>, PgError> {
        conn.query_opt(
            "\
            select file_entries.id, \
                   file_entries.uid, \
                   file_entries.entries_id, \
                   file_entries.name, \
                   file_entries.mime_type, \
                   file_entries.mime_subtype, \
                   file_entries.mime_parameter, \
                   file_entries.size, \
                   file_entries.created, \
                   file_entries.updated \
            from file_entries \
                left join entries on \
                    file_entries.entries_id = entries.id \
            where entries.entry_date = $1 and \
                  file_entries.id = $2",
            &[date, &file_entry_id]
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

    pub async fn retrieve_file_entry(
        conn: &mut db::DbConn,
        date: &NaiveDate,
        file_entry_id: FileEntryId
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query(
            "\
            select file_entries.id, \
                   file_entries.uid, \
                   file_entries.entries_id, \
                   file_entries.name, \
                   file_entries.mime_type, \
                   file_entries.mime_subtype, \
                   file_entries.mime_parameter, \
                   file_entries.size, \
                   file_entries.created, \
                   file_entries.updated \
            from file_entries \
                left join entries on \
                    file_entries.entries_id = entries.id \
            where entries.entry_date = ?1 and \
                  file_entries.id = ?2"
        )
            .bind(date)
            .bind(file_entry_id)
            .fetch_optional(&mut *conn)
            .await
            .map(|result| result.map(|record| Self {
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

    pub async fn retrieve_entry_pg(
        conn: &impl GenericClient,
        entries_id: EntryId
    ) -> Result<Vec<Self>, PgError> {
        let stream = Self::retrieve_entry_stream_pg(conn, &entries_id).await?;
        let mut rtn = Vec::new();

        futures::pin_mut!(stream);

        while let Some(item) = stream.try_next().await? {
            rtn.push(item);
        }

        Ok(rtn)
    }

    pub async fn retrieve_entry(
        conn: &mut db::DbConn,
        entries_id: EntryId
    ) -> Result<Vec<Self>, sqlx::Error> {
        Self::retrieve_entry_stream(conn, entries_id)
            .try_collect()
            .await
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

    pub async fn update_pg(&self, conn: &impl GenericClient) -> Result<(), PgError> {
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

    pub async fn update(&self, conn: &mut db::DbConn) -> Result<(), sqlx::Error> {
        sqlx::query(
            "\
            update file_entries \
            set name = ?2, \
                mime_type = ?3, \
                mime_subtype = ?4, \
                mime_parameter = ?5, \
                size = ?6, \
                updated = ?7 \
            where file_entries.id = ?1"
        )
            .bind(self.id)
            .bind(&self.name)
            .bind(&self.mime_type)
            .bind(&self.mime_subtype)
            .bind(&self.mime_param)
            .bind(self.size)
            .bind(self.updated)
            .execute(&mut *conn)
            .await?;

        Ok(())
    }
}
