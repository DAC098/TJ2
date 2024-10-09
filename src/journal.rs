use chrono::{NaiveDate, DateTime, Utc};
use futures::{Stream, StreamExt, TryStreamExt};
use serde::Serialize;
use sqlx::Row;

use crate::db;
use crate::db::ids::UserId;

#[derive(Debug)]
pub struct JournalEntry {
    pub id: i64,
    pub users_id: UserId,
    pub date: NaiveDate,
    pub title: Option<String>,
    pub contents: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl JournalEntry {
    pub async fn retrieve_date(conn: &mut db::DbConn, users_id: UserId, date: &NaiveDate) -> Result<Option<Self>, sqlx::Error> {
        let result = sqlx::query(
            "\
            select journal.id, \
                   journal.users_id, \
                   journal.entry_date, \
                   journal.title, \
                   journal.contents, \
                   journal.created, \
                   journal.updated \
            from journal \
            where journal.entry_date = ?1 and \
                  journal.users_id = ?2"
        )
            .bind(date)
            .bind(users_id)
            .fetch_optional(&mut *conn)
            .await?;

        if let Some(found) = result {
            Ok(Some(JournalEntry {
                id: found.get(0),
                users_id: found.get(1),
                date: found.get(2),
                title: found.get(3),
                contents: found.get(4),
                created: found.get(5),
                updated: found.get(6),
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Serialize)]
pub struct JournalEntryFull {
    pub id: i64,
    pub users_id: UserId,
    pub date: NaiveDate,
    pub title: Option<String>,
    pub contents: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
    pub tags: Vec<JournalTag>,
}

impl JournalEntryFull {
    pub async fn retrieve_date(conn: &mut db::DbConn, users_id: UserId, date: &NaiveDate) -> Result<Option<Self>, sqlx::Error> {
        if let Some(found) = JournalEntry::retrieve_date(conn, users_id, date).await? {
            let tags = JournalTag::retrieve_date(conn, users_id, date)
                .await?;

            Ok(Some(JournalEntryFull {
                id: found.id,
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
pub struct JournalTag {
    pub key: String,
    pub value: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl JournalTag {
    pub fn retrieve_journal_stream<'a>(
        conn: &'a mut db::DbConn,
        journal_id: i64
    ) -> impl Stream<Item = Result<Self, sqlx::Error>> + 'a {
        sqlx::query(
            "\
            select journal_tags.key, \
                   journal_tags.value, \
                   journal_tags.created, \
                   journal_tags.updated \
            from journal_tags \
            where journal_tags.journal_id = ?1"
        )
            .bind(journal_id)
            .fetch(&mut *conn)
            .map(|res| res.map(|record| JournalTag {
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
            select journal_tags.key, \
                   journal_tags.value, \
                   journal_tags.created, \
                   journal_tags.updated \
            from journal_tags \
                left join journal on \
                    journal_tags.journal_id = journal.id \
            where journal.entry_date = ?1 and \
                  journal.users_id = ?2"
        )
            .bind(date)
            .bind(users_id)
            .fetch(&mut *conn)
            .map(|res| res.map(|record| JournalTag {
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
