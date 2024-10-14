use std::collections::HashMap;

use axum::extract::{Path, Request};
use axum::http::{StatusCode, Uri, HeaderMap};
use axum::response::{IntoResponse, Response};
use chrono::{NaiveDate, Utc, DateTime};
use futures::StreamExt;
use serde::{Serialize, Deserialize};
use sqlx::{QueryBuilder, Row};

use crate::state;
use crate::db;
use crate::db::ids::{EntryId, EntryUid, JournalId, UserId};
use crate::error::{self, Context};
use crate::journal::{Journal, EntryTag, Entry, EntryFull};
use crate::router::body;
use crate::router::macros;
use crate::sec::authz::{Scope, Ability, has_permission, has_permission_ref};

macro_rules! perm_check {
    ($conn:expr, $initiator:expr, $journal:expr, $scope:expr, $ability:expr) => {
        let perm_check = if $journal.users_id == $initiator.user.id {
            has_permission(
                $conn,
                $initiator.user.id,
                $scope,
                $ability,
            )
                .await
                .context("failed to retrieve permissiosn for user")?
        } else {
            has_permission_ref(
                $conn,
                $initiator.user.id,
                $scope,
                $ability,
                $journal.id
            )
                .await
                .context("failed to retrieve permissions for user")?
        };

        if !perm_check {
            return Ok(StatusCode::UNAUTHORIZED.into_response());
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EntryPartial {
    pub id: EntryId,
    pub uid: EntryUid,
    pub journals_id: JournalId,
    pub users_id: UserId,
    pub title: Option<String>,
    pub date: NaiveDate,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
    pub tags: HashMap<String, Option<String>>,
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

    let result = Journal::retrieve_default(&mut conn, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    perm_check!(&mut conn, initiator, journal, Scope::Entries, Ability::Read);

    let mut fut_entries = sqlx::query(
        "\
        with search_entries as ( \
            select * \
            from entries \
            where entries.users_id = ?1 and \
                  entries.journals_id = ?2 \
        ) \
        select search_entries.id, \
               search_entries.uid, \
               search_entries.journals_id, \
               search_entries.users_id, \
               search_entries.title, \
               search_entries.entry_date, \
               search_entries.created, \
               search_entries.updated, \
               entry_tags.key, \
               entry_tags.value
        from search_entries \
            left join entry_tags on \
                search_entries.id = entry_tags.entries_id \
        order by search_entries.entry_date desc"
    )
        .bind(initiator.user.id)
        .bind(journal.id)
        .fetch(&mut *conn);

    let mut found = Vec::new();
    let mut current: Option<EntryPartial> = None;

    while let Some(try_record) = fut_entries.next().await {
        let record = try_record.context("failed to retrieve journal entry")?;
        let key: Option<String> = record.get(8);
        let value: Option<String> = record.get(9);

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

                let mut swapping = EntryPartial {
                    id,
                    uid: record.get(1),
                    journals_id: record.get(2),
                    users_id: record.get(3),
                    title: record.get(4),
                    date: record.get(5),
                    created: record.get(6),
                    updated: record.get(7),
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

            current = Some(EntryPartial {
                id: record.get(0),
                uid: record.get(1),
                journals_id: record.get(2),
                users_id: record.get(3),
                title: record.get(4),
                date: record.get(5),
                created: record.get(6),
                updated: record.get(7),
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

    let Some(date) = date else {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    let mut conn = state.acquire_conn().await?;

    let initiator = macros::require_initiator!(&mut conn, &headers, Some(uri));

    let result = Journal::retrieve_default(&mut conn, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    perm_check!(&mut conn, initiator, journal, Scope::Entries, Ability::Read);

    let result = EntryFull::retrieve_date(
        &mut conn,
        journal.id,
        initiator.user.id,
        &date
    )
        .await
        .context("failed to retrieve journal entry for date")?;

    let Some(entry) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    tracing::debug!("entry: {entry:#?}");

    Ok(body::Json(entry).into_response())
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

    let result = Journal::retrieve_default(&mut conn, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    perm_check!(&mut conn, initiator, journal, Scope::Entries, Ability::Create);

    let uid = EntryUid::gen();
    let journals_id = journal.id;
    let users_id = initiator.user.id;
    let entry_date = json.date;
    let title = opt_non_empty_str(json.title);
    let contents = opt_non_empty_str(json.contents);
    let created = Utc::now();

    let id: EntryId = {
        let result = sqlx::query(
            "\
            insert into entries (uid, journals_id, users_id, entry_date, title, contents, created) \
            values (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
            returning id"
        )
            .bind(&uid)
            .bind(journals_id)
            .bind(users_id)
            .bind(entry_date)
            .bind(&title)
            .bind(&contents)
            .bind(created)
            .fetch_one(&mut *conn)
            .await
            .context("failed to insert entry into database")?;

        result.get(0)
    };

    let mut first = true;
    let mut tags: Vec<EntryTag> = Vec::new();
    let mut query_builder: QueryBuilder<db::Db> = QueryBuilder::new(
        "insert into entry_tags (entries_id, key, value, created) values "
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

        tags.push(EntryTag {
            key: key.clone(),
            value: value.clone(),
            created,
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

    let entry = EntryFull {
        id,
        uid,
        journals_id,
        users_id,
        date: entry_date,
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
    let result = Journal::retrieve_default(&mut conn, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    perm_check!(&mut conn, initiator, journal, Scope::Entries, Ability::Update);

    let result = Entry::retrieve_date(
        &mut conn,
        journal.id,
        initiator.user.id,
        &date
    )
        .await
        .context("failed to retrieve journal entry by date")?;

    let Some(entry) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    tracing::debug!("entry: {entry:#?}");

    let entry_date = json.date;
    let title = opt_non_empty_str(json.title);
    let contents = opt_non_empty_str(json.contents);
    let updated = Utc::now();

    sqlx::query(
        "\
        update entries \
        set entry_date = ?2, \
            title = ?3, \
            contents = ?4, \
            updated = ?5 \
        where id = ?1"
    )
        .bind(entry.id)
        .bind(entry_date)
        .bind(&title)
        .bind(&contents)
        .bind(updated)
        .execute(&mut *conn)
        .await
        .context("failed to update journal entry")?;

    let tags = {
        let mut tags: Vec<EntryTag> = Vec::new();
        let mut current_tags: HashMap<String, EntryTag> = HashMap::new();

        {
            let mut tag_stream = EntryTag::retrieve_entry_stream(&mut conn, entry.id);
            while let Some(tag_result) = tag_stream.next().await {
                let tag = tag_result.context("failed to retrieve journal tag")?;

                current_tags.insert(tag.key.clone(), tag);
            }
        }

        let mut changed = false;
        let mut upsert_first = true;
        let mut upsert_tags: QueryBuilder<db::Db> = QueryBuilder::new(
            "\
            insert into entry_tags (entries_id, key, value, created) values "
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
                tags.push(EntryTag {
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
            separated.push_bind(entry.id);
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
                "delete from entry_tags where entries_id = "
            );
            delete_tags.push_bind(entry.id);
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

    let entry = EntryFull {
        id: entry.id,
        uid: entry.uid,
        journals_id: entry.journals_id,
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
    let result = Journal::retrieve_default(&mut conn, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    perm_check!(&mut conn, initiator, journal, Scope::Entries, Ability::Delete);

    let result = EntryFull::retrieve_date(
        &mut conn,
        journal.id,
        initiator.user.id,
        &date
    )
        .await
        .context("failed to retrieve journal entry by date")?;

    let Some(entry) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let tags = sqlx::query("delete from entry_tags where entries_id = ?1")
        .bind(entry.id)
        .execute(&mut *conn)
        .await
        .context("failed to delete tags for journal entry")?
        .rows_affected();

    if tags != entry.tags.len() as u64 {
        tracing::warn!("dangling tags for journal entry");
    }

    let entry = sqlx::query("delete from entries where id = ?1")
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
