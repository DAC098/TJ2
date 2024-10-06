use std::collections::HashMap;

use axum::extract::{Path, Request};
use axum::http::{StatusCode, Uri, HeaderMap};
use axum::response::{IntoResponse, Response};
use chrono::{NaiveDate, Utc, DateTime};
use futures::StreamExt;
use serde::{Serialize, Deserialize};
use sqlx::Row;

use crate::state;
use crate::db;
use crate::error::{self, Context};
use crate::router::body;
use crate::router::macros;

#[derive(Debug, Serialize)]
pub struct JournalEntry {
    pub id: i64,
    pub title: Option<String>,
    pub date: NaiveDate,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
    pub tags: HashMap<String, Option<String>>,
}

#[derive(Debug, Serialize)]
pub struct JournalEntryFull {
    pub id: i64,
    pub users_id: i64,
    pub date: NaiveDate,
    pub title: Option<String>,
    pub contents: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
    pub tags: Vec<JournalTag>,
}

impl JournalEntryFull {
    async fn retrieve_date(conn: &mut db::DbConn, users_id: i64, date: &NaiveDate) -> Result<Option<Self>, error::Error> {
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
            .await
            .context("failed to retrieve journal entry by date")?;

        if let Some(found) = result {
            let tags = JournalTag::retrieve_date(conn, users_id, date)
                .await
                .context("failed to retrieve tags for journal entry by date")?;

            Ok(Some(JournalEntryFull {
                id: found.get(0),
                users_id: found.get(1),
                date: found.get(2),
                title: found.get(3),
                contents: found.get(4),
                created: found.get(5),
                updated: found.get(6),
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
    async fn retrieve_date(conn: &mut db::DbConn, users_id: i64, date: &NaiveDate) -> Result<Vec<Self>, error::Error> {
        let mut stream = sqlx::query(
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
            .fetch(&mut *conn);

        let mut tags = Vec::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record.context("failed to retrieve journal tag by date")?;

            tags.push(JournalTag {
                key: record.get(0),
                value: record.get(1),
                created: record.get(2),
                updated: record.get(3),
            });
        }

        Ok(tags)
    }
}

pub async fn retrieve_entries(
    state: state::SharedState,
    req: Request,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), req.headers());

    let mut conn = state.acquire_conn().await?;

    let initiator = macros::require_initiator!(
        &mut conn,
        req.headers(),
        Some(req.uri().clone())
    );

    let mut fut_entries = sqlx::query(
        "\
        with search_entries as ( \
            select * \
            from journal \
            where journal.users_id = ?1 \
        ) \
        select search_entries.id, \
               search_entries.title, \
               search_entries.entry_date, \
               search_entries.created, \
               search_entries.updated, \
               journal_tags.key, \
               journal_tags.value
        from search_entries \
            left join journal_tags on \
                search_entries.id = journal_tags.journal_id \
        order by search_entries.entry_date desc"
    )
        .bind(initiator.users_id)
        .fetch(&mut *conn);

    let mut found = Vec::new();
    let mut current: Option<JournalEntry> = None;

    while let Some(try_record) = fut_entries.next().await {
        let record = try_record.context("failed to retrieve journal entry")?;
        let key: Option<String> = record.get(5);
        let value: Option<String> = record.get(6);

        if let Some(curr) = &mut current {
            let id = record.get(0);

            if curr.id == id {
                if let Some(key) = key {
                    curr.tags.insert(key, value);
                }
            } else {
                let tags = if let Some(key) = key {
                    HashMap::from([(key, value)])
                } else {
                    HashMap::new()
                };

                let mut swapping = JournalEntry {
                    id,
                    title: record.get(1),
                    date: record.get(2),
                    created: record.get(3),
                    updated: record.get(4),
                    tags
                };

                std::mem::swap(&mut swapping, curr);

                found.push(swapping);
            }
        } else {
            let tags = if let Some(key) = key {
                HashMap::from([(key, value)])
            } else {
                HashMap::new()
            };

            current = Some(JournalEntry {
                id: record.get(0),
                title: record.get(1),
                date: record.get(2),
                created: record.get(3),
                updated: record.get(4),
                tags
            });
        }
    }

    if let Some(curr) = current {
        found.push(curr);
    }

    Ok(body::Json(found).into_response())
}

#[derive(Debug, Deserialize)]
pub struct EntryDate {
    date: Option<NaiveDate>
}

pub async fn retrieve_entry(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
    Path(EntryDate { date }): Path<EntryDate>,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let mut conn = state.acquire_conn().await?;

    let initiator = macros::require_initiator!(&mut conn, &headers, Some(uri));

    if let Some(date) = &date {
        if let Some(entry) = JournalEntryFull::retrieve_date(&mut conn, initiator.users_id, date).await? {
            tracing::debug!("entry: {entry:#?}");

            Ok(body::Json(entry).into_response())
        } else {
            Ok(StatusCode::NOT_FOUND.into_response())
        }
    } else {
        Ok(StatusCode::BAD_REQUEST.into_response())
    }
}

#[derive(Debug, Deserialize)]
pub struct EntryBody {
    date: NaiveDate,
    title: Option<String>,
    contents: Option<String>,
    tags: HashMap<String, Option<String>>,
}

pub async fn upsert_entry(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
    Path(EntryDate { date }): Path<EntryDate>,
    body::Json(data): body::Json<EntryBody>,
) -> Result<Response, error::Error> {
    let mut conn = state.begin_conn().await?;

    let initiator = macros::require_initiator!(&mut conn, &headers, Some(uri));

    tracing::debug!("entry body: {data:#?}");

    if let Some(date) = &date {
        let Some(entry) = JournalEntryFull::retrieve_date(&mut conn, initiator.users_id, date).await? else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };

        tracing::debug!("entry found: {entry:#?}");
    } else {
        
    }

    conn.commit()
        .await
        .context("failed commit changes to journal entry")?;

    Ok(body::Json("okay").into_response())
}
