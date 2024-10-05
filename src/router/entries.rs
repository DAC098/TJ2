use std::collections::HashMap;

use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use chrono::{NaiveDate, Utc, DateTime};
use futures::StreamExt;
use serde::Serialize;
use sqlx::Row;

use crate::state;
use crate::error::{self, Context};
use crate::router::body;
use crate::router::macros;

#[derive(Debug, Serialize)]
pub struct JournalEntry {
    pub id: i64,
    pub date: NaiveDate,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
    pub tags: HashMap<String, Option<String>>,
}

pub async fn retrieve_entries(
    state: state::SharedState,
    req: Request,
) -> Result<Response, error::Error> {
    let mut conn = state.db()
        .acquire()
        .await
        .context("failed to retrieve database connection")?;

    macros::res_if_html!(state.templates(), req.headers());

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
        let key: Option<String> = record.get(4);
        let value: Option<String> = record.get(5);

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
                    date: record.get(1),
                    created: record.get(2),
                    updated: record.get(3),
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
                date: record.get(1),
                created: record.get(2),
                updated: record.get(3),
                tags
            });
        }
    }

    if let Some(curr) = current {
        found.push(curr);
    }

    Ok(body::Json(found).into_response())
}
