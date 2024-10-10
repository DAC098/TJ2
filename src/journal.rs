use chrono::{NaiveDate, DateTime, Utc};
use futures::{Stream, StreamExt, TryStreamExt};
use serde::Serialize;
use sqlx::Row;

use crate::db;
use crate::db::ids::{EntryId, EntryUid, JournalId, JournalUid, UserId};

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
pub struct EntryFull {
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
}

impl EntryFull {
    pub async fn retrieve_date(
        conn: &mut db::DbConn,
        journals_id: JournalId,
        users_id: UserId,
        date: &NaiveDate
    ) -> Result<Option<Self>, sqlx::Error> {
        if let Some(found) = Entry::retrieve_date(conn, journals_id, users_id, date).await? {
            let tags = EntryTag::retrieve_date(conn, users_id, date)
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
                tags
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
    pub fn retrieve_entry_stream<'a>(
        conn: &'a mut db::DbConn,
        entry_id: EntryId
    ) -> impl Stream<Item = Result<Self, sqlx::Error>> + 'a {
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

    pub async fn retrieve_date(conn: &mut db::DbConn, users_id: UserId, date: &NaiveDate) -> Result<Vec<Self>, sqlx::Error> {
        Self::retrieve_date_stream(conn, users_id, date)
            .try_collect()
            .await
    }
}
