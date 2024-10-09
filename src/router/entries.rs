use std::collections::HashMap;

use axum::extract::{Path, Request};
use axum::http::{StatusCode, Uri, HeaderMap};
use axum::response::{IntoResponse, Response};
use chrono::{NaiveDate, Utc, DateTime};
use futures::{Stream, StreamExt, TryStreamExt};
use serde::{Serialize, Deserialize};
use sqlx::{QueryBuilder, Row};

use crate::state;
use crate::db;
use crate::db::ids::UserId;
use crate::error::{self, Context};
use crate::router::body;
use crate::router::macros;

#[derive(Debug, Serialize)]
pub struct JournalEntryPartial {
    pub id: i64,
    pub title: Option<String>,
    pub date: NaiveDate,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
    pub tags: HashMap<String, Option<String>>,
}

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
    async fn retrieve_date(conn: &mut db::DbConn, users_id: UserId, date: &NaiveDate) -> Result<Option<Self>, error::Error> {
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
    async fn retrieve_date(conn: &mut db::DbConn, users_id: UserId, date: &NaiveDate) -> Result<Option<Self>, error::Error> {
        if let Some(found) = JournalEntry::retrieve_date(conn, users_id, date).await? {
            let tags = JournalTag::retrieve_date(conn, users_id, date)
                .await
                .context("failed to retrieve tags for journal entry by date")?;

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
    fn retrieve_journal_stream<'a>(
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

    fn retrieve_date_stream<'a>(
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

    async fn retrieve_date(conn: &mut db::DbConn, users_id: UserId, date: &NaiveDate) -> Result<Vec<Self>, error::Error> {
        Self::retrieve_date_stream(conn, users_id, date)
            .map_err(|err| error::Error::context_source(
                "failed to retrieve tags",
                err
            ))
            .try_collect()
            .await
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
        .bind(initiator.user.id)
        .fetch(&mut *conn);

    let mut found = Vec::new();
    let mut current: Option<JournalEntryPartial> = None;

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

                let mut swapping = JournalEntryPartial {
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

            current = Some(JournalEntryPartial {
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
pub struct MaybeEntryDate {
    date: Option<NaiveDate>
}

#[derive(Debug, Deserialize)]
pub struct EntryDate {
    date: NaiveDate
}

pub async fn retrieve_entry(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
    Path(MaybeEntryDate { date }): Path<MaybeEntryDate>,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let mut conn = state.acquire_conn().await?;

    let initiator = macros::require_initiator!(&mut conn, &headers, Some(uri));

    if let Some(date) = &date {
        if let Some(entry) = JournalEntryFull::retrieve_date(&mut conn, initiator.user.id, date).await? {
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
    tags: Vec<EntryTagBody>,
}

#[derive(Debug, Deserialize)]
pub struct EntryTagBody {
    key: String,
    value: Option<String>,
}

fn non_empty_str(given: String) -> Option<String> {
    let trimmed = given.trim();

    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn opt_non_empty_str(given: Option<String>) -> Option<String> {
    if let Some(value) = given {
        non_empty_str(value)
    } else {
        None
    }
}

pub async fn create_entry(
    state: state::SharedState,
    headers: HeaderMap,
    body::Json(json): body::Json<EntryBody>,
) -> Result<Response, error::Error> {
    let mut conn = state.begin_conn().await?;

    let initiator = macros::require_initiator!(&mut conn, &headers, None::<Uri>);

    let entry_date = json.date;
    let users_id = initiator.user.id;
    let title = opt_non_empty_str(json.title);
    let contents = opt_non_empty_str(json.contents);
    let created = Utc::now();

    let id: i64 = {
        let result = sqlx::query(
            "\
            insert into journal (users_id, entry_date, title, contents, created) \
            values (?1, ?2, ?3, ?4, ?5) \
            returning id"
        )
            .bind(users_id)
            .bind(&entry_date)
            .bind(&title)
            .bind(&contents)
            .bind(created)
            .fetch_one(&mut *conn)
            .await
            .context("failed to insert entry into database")?;

        result.get(0)
    };

    let mut first = true;
    let mut tags: Vec<JournalTag> = Vec::new();
    let mut query_builder: QueryBuilder<db::Db> = QueryBuilder::new(
        "insert into journal_tags (journal_id, key, value, created) values "
    );

    for tag in json.tags {
        let Some(key) = non_empty_str(tag.key) else {
            continue;
        };
        let value = opt_non_empty_str(tag.value);

        if first {
            query_builder.push("(");
            first = false;
        } else {
            query_builder.push(", (");
        }

        tags.push(JournalTag {
            key: key.clone(),
            value: value.clone(),
            created: created.clone(),
            updated: None
        });

        let mut separated = query_builder.separated(", ");
        separated.push_bind(id);
        separated.push_bind(key);
        separated.push_bind(value);
        separated.push_bind(created);
        separated.push_unseparated(")");
    }

    let query = query_builder.build();

    query.execute(&mut *conn)
        .await
        .context("failed to commit tags")?;

    conn.commit()
        .await
        .context("failed to commit changes to journal entry")?;

    let entry = JournalEntryFull {
        id,
        date: entry_date,
        users_id,
        title,
        contents,
        created,
        updated: None,
        tags
    };

    Ok((
        StatusCode::CREATED,
        body::Json(entry),
    ).into_response())
}

pub async fn update_entry(
    state: state::SharedState,
    headers: HeaderMap,
    Path(EntryDate { date }): Path<EntryDate>,
    body::Json(json): body::Json<EntryBody>,
) -> Result<Response, error::Error> {
    let mut conn = state.begin_conn().await?;

    let initiator = macros::require_initiator!(&mut conn, &headers, None::<Uri>);

    let Some(entry) = JournalEntry::retrieve_date(&mut conn, initiator.user.id, &date).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    tracing::debug!("entry: {entry:#?}");

    let entry_date = json.date;
    let title = opt_non_empty_str(json.title);
    let contents = opt_non_empty_str(json.contents);
    let updated = Utc::now();

    sqlx::query(
        "\
        update journal \
        set entry_date = ?2, \
            title = ?3, \
            contents = ?4, \
            updated = ?5 \
        where id = ?1"
    )
        .bind(&entry.id)
        .bind(entry_date)
        .bind(&title)
        .bind(&contents)
        .bind(updated)
        .execute(&mut *conn)
        .await
        .context("failed to update journal entry")?;

    let tags = {
        let mut tags: Vec<JournalTag> = Vec::new();
        let mut current_tags: HashMap<String, JournalTag> = HashMap::new();

        {
            let mut tag_stream = JournalTag::retrieve_journal_stream(&mut conn, entry.id);
            while let Some(tag_result) = tag_stream.next().await {
                let tag = tag_result.context("failed to retrieve journal tag")?;

                current_tags.insert(tag.key.clone(), tag);
            }
        }

        let mut changed = false;
        let mut upsert_first = true;
        let mut upsert_tags: QueryBuilder<db::Db> = QueryBuilder::new(
            "\
            insert into journal_tags (journal_id, key, value, created) values "
        );

        for tag in json.tags {
            let Some(key) = non_empty_str(tag.key) else {
                continue;
            };
            let value = opt_non_empty_str(tag.value);

            if let Some(mut found) = current_tags.remove(&key) {
                if found.value != value {
                    found.value = value.clone();
                    found.updated = Some(updated);

                    tags.push(found);

                    changed = true;
                } else {
                    tags.push(found);

                    continue;
                }
            } else {
                tags.push(JournalTag {
                    key: key.clone(),
                    value: value.clone(),
                    created: updated,
                    updated: None,
                });

                changed = true;
            }

            if upsert_first {
                upsert_tags.push("(");
                upsert_first = false;
            } else {
                upsert_tags.push(", (");
            }

            let mut separated = upsert_tags.separated(", ");
            separated.push_bind(&entry.id);
            separated.push_bind(key);
            separated.push_bind(value);
            separated.push_bind(updated);
            separated.push_unseparated(")");
        }

        if changed {
            upsert_tags.push(" on conflict do update set \
                value = EXCLUDED.value, \
                updated = EXCLUDED.created");

            let upsert_query = upsert_tags.build();

            upsert_query.execute(&mut *conn)
                .await
                .context("failed to upsert tags for journal")?;
        }

        if !current_tags.is_empty() {
            let mut delete_tags: QueryBuilder<db::Db> = QueryBuilder::new(
                "delete from journal_tags where journal_id = "
            );
            delete_tags.push_bind(&entry.id);
            delete_tags.push(" and key in (");

            let mut separated = delete_tags.separated(", ");

            for (key, _) in current_tags {
                separated.push_bind(key);
            }

            separated.push_unseparated(")");

            let delete_query = delete_tags.build();

            delete_query.execute(&mut *conn)
                .await
                .context("failed to delete tags for journal")?
                .rows_affected();
        }

        tags
    };

    conn.commit()
        .await
        .context("failed commit changes to journal entry")?;

    let entry = JournalEntryFull {
        id: entry.id,
        users_id: entry.users_id,
        date: entry_date,
        title,
        contents,
        created: entry.created,
        updated: Some(updated),
        tags
    };

    Ok(body::Json(entry).into_response())
}

pub async fn delete_entry(
    state: state::SharedState,
    headers: HeaderMap,
    Path(EntryDate { date }): Path<EntryDate>,
) -> Result<Response, error::Error> {
    let mut conn = state.begin_conn().await?;

    let initiator = macros::require_initiator!(&mut conn, &headers, None::<Uri>);

    let Some(entry) = JournalEntryFull::retrieve_date(&mut conn, initiator.user.id, &date).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let tags = sqlx::query("delete from journal_tags where journal_id = ?1")
        .bind(entry.id)
        .execute(&mut *conn)
        .await
        .context("failed to delete tags for journal entry")?
        .rows_affected();

    if tags != entry.tags.len() as u64 {
        tracing::warn!("dangling tags for journal entry");
    }

    let entry = sqlx::query("delete from journal where id = ?1")
        .bind(entry.id)
        .execute(&mut *conn)
        .await
        .context("failed to delete journal entry")?
        .rows_affected();

    if entry != 1 {
        tracing::warn!("did not find journal entry?");
    }

    conn.commit()
        .await
        .context("failed to commit changes to journal")?;

    Ok(body::Json(entry).into_response())
}
