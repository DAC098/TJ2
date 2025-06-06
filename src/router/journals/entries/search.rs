use std::collections::{BTreeMap, HashMap};

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, NaiveDate, Utc};
use deadpool_postgres::Transaction;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{CustomFieldId, EntryId, EntryUid, JournalId, UserId};
use crate::journal::{assert_permission, custom_field, CustomField, Journal};
use crate::net::body;
use crate::net::Error;
use crate::sec::authn::Initiator;
use crate::sec::authz::{Ability, Scope};
use crate::state;

#[derive(Debug, Deserialize)]
pub struct JournalPath {
    journals_id: JournalId,
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
    pub tags: BTreeMap<String, Option<String>>,
    pub custom_fields: HashMap<CustomFieldId, custom_field::Value>,
}

#[derive(Debug, Serialize)]
pub struct CustomFieldPartial {
    id: CustomFieldId,
    name: String,
    description: Option<String>,
    config: custom_field::Type,
}

#[derive(Debug)]
struct EntryRow {
    id: EntryId,
    uid: EntryUid,
    journals_id: JournalId,
    users_id: UserId,
    title: Option<String>,
    date: NaiveDate,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
}

#[derive(Debug)]
struct TagRow {
    key: String,
    value: Option<String>,
}

#[derive(Debug)]
struct CFValueRow {
    custom_fields_id: CustomFieldId,
    value: custom_field::Value,
}

#[derive(Debug, Serialize)]
pub struct SearchResults {
    total: u64,
    entries: Vec<EntryPartial>,
    custom_fields: Option<Vec<CustomFieldPartial>>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum SearchEntriesError {
    JournalNotFound,
}

impl IntoResponse for SearchEntriesError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

#[axum::debug_handler]
pub async fn retrieve_entries(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(JournalPath { journals_id }): Path<JournalPath>,
) -> Result<body::Json<SearchResults>, Error<SearchEntriesError>> {
    body::assert_html(state.templates(), &headers)?;

    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let journal = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
        .await?
        .ok_or(Error::Inner(SearchEntriesError::JournalNotFound))?;

    assert_permission(
        &transaction,
        &initiator,
        &journal,
        Scope::Entries,
        Ability::Read,
    )
    .await?;

    let custom_fields = if true {
        Some(retrieve_journal_cfs(&transaction, &journal.id).await?)
    } else {
        None
    };

    let rtn = multi_query_search(&transaction, &journal.id, 75).await?;

    transaction.rollback().await?;

    Ok(body::Json(SearchResults {
        total: rtn.len() as u64,
        entries: rtn,
        custom_fields,
    }))
}

async fn retrieve_journal_cfs(
    conn: &impl db::GenericClient,
    journals_id: &JournalId,
) -> Result<Vec<CustomFieldPartial>, Error<SearchEntriesError>> {
    let stream = CustomField::retrieve_journal_stream(conn, journals_id).await?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let CustomField {
            id,
            name,
            description,
            config,
            ..
        } = try_record?;

        rtn.push(CustomFieldPartial {
            id,
            name,
            description,
            config,
        });
    }

    Ok(rtn)
}

async fn multi_query_search(
    conn: &Transaction<'_>,
    journals_id: &JournalId,
    batch_size: i32,
) -> Result<Vec<EntryPartial>, Error<SearchEntriesError>> {
    let params: db::ParamsArray<'_, 1> = [journals_id];

    let portal = conn
        .bind_raw(
            "\
        select entries.id, \
               entries.uid, \
               entries.journals_id, \
               entries.users_id, \
               entries.title, \
               entries.entry_date, \
               entries.created, \
               entries.updated \
        from entries \
        where entries.journals_id = $1 \
        order by entries.entry_date desc",
            params,
        )
        .await?;

    let mut rtn = Vec::new();

    loop {
        let stream = conn
            .query_portal_raw(&portal, batch_size)
            .await?
            .map(|result| {
                result.map(|row| EntryRow {
                    id: row.get(0),
                    uid: row.get(1),
                    journals_id: row.get(2),
                    users_id: row.get(3),
                    title: row.get(4),
                    date: row.get(5),
                    created: row.get(6),
                    updated: row.get(7),
                })
            });

        futures::pin_mut!(stream);

        while let Some(result) = stream.next().await {
            let row = result?;

            let (tags_res, custom_fields_res) = tokio::join!(
                multi_query_tags(conn, &row.id),
                multi_query_cfs(conn, &row.id)
            );

            rtn.push(EntryPartial {
                id: row.id,
                uid: row.uid,
                journals_id: row.journals_id,
                users_id: row.users_id,
                title: row.title,
                date: row.date,
                created: row.created,
                updated: row.updated,
                tags: tags_res?,
                custom_fields: custom_fields_res?,
            });
        }

        if let Some(affected) = stream.get_ref().rows_affected() {
            if affected == 0 {
                break;
            }
        }
    }

    Ok(rtn)
}

async fn multi_query_tags(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
) -> Result<BTreeMap<String, Option<String>>, Error<SearchEntriesError>> {
    let params: db::ParamsArray<'_, 1> = [entries_id];
    let stream = conn
        .query_raw(
            "\
        select entry_tags.key, \
               entry_tags.value \
        from entry_tags \
            left join entries on \
                entry_tags.entries_id = entries.id \
        where entries.id = $1 \
        order by entry_tags.key",
            params,
        )
        .await?
        .map(|result| {
            result.map(|row| TagRow {
                key: row.get(0),
                value: row.get(1),
            })
        });

    futures::pin_mut!(stream);

    let mut tags = BTreeMap::new();

    while let Some(result) = stream.next().await {
        let record = result?;

        tags.insert(record.key, record.value);
    }

    Ok(tags)
}

async fn multi_query_cfs(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
) -> Result<HashMap<CustomFieldId, custom_field::Value>, Error<SearchEntriesError>> {
    let params: db::ParamsArray<'_, 1> = [entries_id];
    let stream = conn
        .query_raw(
            "\
        select custom_field_entries.custom_fields_id, \
               custom_field_entries.value \
        from custom_field_entries \
            left join entries on \
                custom_field_entries.entries_id = entries.id \
        where entries.id = $1 \
        order by entries.entry_date desc",
            params,
        )
        .await?
        .map(|result| {
            result.map(|row| CFValueRow {
                custom_fields_id: row.get(0),
                value: row.get(1),
            })
        });

    futures::pin_mut!(stream);

    let mut rtn = HashMap::new();

    while let Some(result) = stream.next().await {
        let record = result?;

        rtn.insert(record.custom_fields_id, record.value);
    }

    Ok(rtn)
}
