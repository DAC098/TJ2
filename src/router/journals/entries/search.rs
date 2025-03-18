use std::collections::{HashMap, BTreeMap};

use axum::extract::Path;
use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};
use chrono::{NaiveDate, Utc, DateTime};
use futures::StreamExt;
use serde::{Serialize, Deserialize};
use deadpool_postgres::Transaction;

use crate::db;
use crate::db::ids::{
    EntryId,
    EntryUid,
    JournalId,
    UserId,
    CustomFieldId
};
use crate::error::{self, Context};
use crate::journal::{
    custom_field,
    Journal,
    CustomField,
};
use crate::router::body;
use crate::router::macros;
use crate::sec::authn::Initiator;
use crate::sec::authz::{Scope, Ability};
use crate::state;

use super::auth;

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
#[serde(tag = "type")]
pub enum RetrieveResults {
    JournalNotFound,
    Successful {
        total: u64,
        entries: Vec<EntryPartial>,
        custom_fields: Option<Vec<CustomFieldPartial>>,
    }
}

impl IntoResponse for RetrieveResults {
    fn into_response(self) -> Response {
        match &self {
            Self::JournalNotFound => (
                StatusCode::NOT_FOUND,
                body::Json(self)
            ).into_response(),
            Self::Successful { .. } => (
                StatusCode::OK,
                body::Json(self)
            ).into_response()
        }
    }
}

pub async fn retrieve_entries(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(JournalPath { journals_id }): Path<JournalPath>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create database transaction")?;

    macros::res_if_html!(state.templates(), &headers);

    let result = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(RetrieveResults::JournalNotFound.into_response());
    };

    auth::perm_check!(&transaction, initiator, journal, Scope::Entries, Ability::Read);

    let custom_fields = if true {
        Some(retrieve_journal_cfs(&transaction, journal.id()).await?)
    } else {
        None
    };

    let rtn = multi_query_search(&transaction, journal.id(), 75).await?;

    transaction.rollback()
        .await
        .context("failed to rollback journal entries search transaction")?;

    Ok(RetrieveResults::Successful {
        total: rtn.len() as u64,
        entries: rtn,
        custom_fields,
    }.into_response())
}

async fn retrieve_journal_cfs(
    conn: &impl db::GenericClient,
    journals_id: &JournalId,
) -> Result<Vec<CustomFieldPartial>, error::Error> {
    let stream = CustomField::retrieve_journal_stream(conn, journals_id)
        .await
        .context("failed to retrieve journal custom fields")?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let CustomField {
            id,
            name,
            description,
            config,
            ..
        } = try_record.context("failed to retrieve journal custom field record")?;

        rtn.push(CustomFieldPartial {
            id,
            name,
            description,
            config
        });
    }

    Ok(rtn)
}

async fn multi_query_search(
    conn: &Transaction<'_>,
    journals_id: &JournalId,
    batch_size: i32,
) -> Result<Vec<EntryPartial>, error::Error> {
    let params: db::ParamsArray<'_, 1> = [journals_id];

    let portal = conn.bind_raw(
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
        params
    )
        .await
        .context("failed to retrieve journal entries")?;

    let mut rtn = Vec::new();

    loop {
        let stream = conn.query_portal_raw(&portal, batch_size)
            .await
            .context("failed to retrieve journal entries portal chunk")?
            .map(|result| match result {
                Ok(row) => Ok(EntryRow {
                    id: row.get(0),
                    uid: row.get(1),
                    journals_id: row.get(2),
                    users_id: row.get(3),
                    title: row.get(4),
                    date: row.get(5),
                    created: row.get(6),
                    updated: row.get(7),
                }),
                Err(err) => Err(error::Error::context_source(
                    "failed to retrieve entry record",
                    err
                ))
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
    entries_id: &EntryId
) -> Result<BTreeMap<String, Option<String>>, error::Error> {
    let params: db::ParamsArray<'_, 1> = [entries_id];
    let stream = conn.query_raw(
        "\
        select entry_tags.key, \
               entry_tags.value \
        from entry_tags \
            left join entries on \
                entry_tags.entries_id = entries.id \
        where entries.id = $1 \
        order by entry_tags.key",
        params
    )
        .await
        .context("failed to retrieve entry tags")?
        .map(|result| match result {
            Ok(row) => Ok(TagRow {
                key: row.get(0),
                value: row.get(1),
            }),
            Err(err) => Err(error::Error::context_source(
                "failed to retrieve tag record",
                err
            ))
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
) -> Result<HashMap<CustomFieldId, custom_field::Value>, error::Error> {
    let params: db::ParamsArray<'_, 1> = [entries_id];
    let stream = conn.query_raw(
        "\
        select custom_field_entries.custom_fields_id, \
               custom_field_entries.value \
        from custom_field_entries \
            left join entries on \
                custom_field_entries.entries_id = entries.id \
        where entries.id = $1 \
        order by entries.entry_date desc",
        params
    )
        .await
        .context("failed to retrieve entry custom_fields")?
        .map(|result| match result {
            Ok(row) => Ok(CFValueRow {
                custom_fields_id: row.get(0),
                value: row.get(1),
            }),
            Err(err) => Err(error::Error::context_source(
                "failed to retrieve cf value record",
                err
            ))
        });

    futures::pin_mut!(stream);

    let mut rtn = HashMap::new();

    while let Some(result) = stream.next().await {
        let record = result?;

        rtn.insert(record.custom_fields_id, record.value);
    }

    Ok(rtn)
}
