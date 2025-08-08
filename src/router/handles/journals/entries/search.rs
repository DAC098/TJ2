use std::cmp;
use std::collections::{BTreeMap, HashMap};
use std::fmt;

use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, NaiveDate, Utc};
use deadpool_postgres::Transaction;
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{CustomFieldId, EntryId, EntryUid, JournalId, UserId};
use crate::journal::{assert_permission, custom_field, sharing, CustomField, Journal};
use crate::net::body;
use crate::net::Error;
use crate::sec::authn::Initiator;
use crate::sec::authz::{Ability, Scope};
use crate::state;

#[derive(Debug, Deserialize)]
pub struct JournalPath {
    journals_id: JournalId,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,

    // used for both page and keyset pagination
    #[serde(default)]
    size: SearchSize,
    // page based pagination
    //#[serde(default)]
    //page: u32,

    // keyset based pagination
    // if present will override page
    //prev: Option<NaiveDate>,
    //#[serde(default)]
    //dir: SearchDir,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[repr(i32)]
enum SearchSize {
    Month = 31,
    HalfYear = 366 / 2,
    Year = 366,
}

impl cmp::PartialEq<i32> for SearchSize {
    fn eq(&self, other: &i32) -> bool {
        let v = *self as i32;

        v.eq(other)
    }
}

impl cmp::PartialOrd<i32> for SearchSize {
    fn partial_cmp(&self, other: &i32) -> Option<cmp::Ordering> {
        let v = *self as i32;

        v.partial_cmp(other)
    }
}

impl<'a> From<&'a SearchSize> for i32 {
    fn from(v: &'a SearchSize) -> Self {
        *v as i32
    }
}

impl<'a> From<&'a SearchSize> for usize {
    fn from(v: &'a SearchSize) -> Self {
        (*v as i32) as usize
    }
}

impl Default for SearchSize {
    fn default() -> Self {
        Self::Year
    }
}

impl fmt::Display for SearchSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self as i32)
    }
}

#[derive(Debug, Deserialize)]
enum SearchDir {
    Frwd,
    Back,
}

impl Default for SearchDir {
    fn default() -> Self {
        Self::Frwd
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
    pub tags: BTreeMap<String, Option<String>>,
    pub custom_fields: HashMap<CustomFieldId, custom_field::Value>,
}

impl
    From<(
        EntryRow,
        BTreeMap<String, Option<String>>,
        HashMap<CustomFieldId, custom_field::Value>,
    )> for EntryPartial
{
    fn from(
        (entry, tags, custom_fields): (
            EntryRow,
            BTreeMap<String, Option<String>>,
            HashMap<CustomFieldId, custom_field::Value>,
        ),
    ) -> Self {
        Self {
            id: entry.id,
            uid: entry.uid,
            journals_id: entry.journals_id,
            users_id: entry.users_id,
            title: entry.title,
            date: entry.date,
            created: entry.created,
            updated: entry.updated,
            tags,
            custom_fields,
        }
    }
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
    InvalidEndDate,
}

impl IntoResponse for SearchEntriesError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::InvalidEndDate => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
        }
    }
}

#[axum::debug_handler]
pub async fn search_entries(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(JournalPath { journals_id }): Path<JournalPath>,
    Query(query): Query<SearchQuery>,
) -> Result<body::Json<SearchResults>, Error<SearchEntriesError>> {
    body::assert_html(state.templates(), &headers)?;

    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let journal = Journal::retrieve(&transaction, &journals_id)
        .await?
        .ok_or(Error::Inner(SearchEntriesError::JournalNotFound))?;

    assert_permission(
        &transaction,
        &initiator,
        &journal,
        Scope::Entries,
        Ability::Read,
        sharing::Ability::EntryRead,
    )
    .await?;

    let custom_fields = if true {
        Some(retrieve_journal_cfs(&transaction, &journal.id).await?)
    } else {
        None
    };

    let rtn = multi_query_search(&transaction, &journal.id, query, 75).await?;

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
    Ok(CustomField::retrieve_journal_stream(conn, journals_id)
        .await?
        .map(|maybe| {
            maybe.map(
                |CustomField {
                     id,
                     name,
                     description,
                     config,
                     ..
                 }| CustomFieldPartial {
                    id,
                    name,
                    description,
                    config,
                },
            )
        })
        .try_collect::<Vec<CustomFieldPartial>>()
        .await?)
}

async fn multi_query_search(
    conn: &Transaction<'_>,
    journals_id: &JournalId,
    SearchQuery {
        start_date,
        end_date,
        size,
        //page,
        //prev,
        //dir,
        ..
    }: SearchQuery,
    batch_size: i32,
) -> Result<Vec<EntryPartial>, Error<SearchEntriesError>> {
    let mut params: db::ParamsVec<'_> = vec![journals_id];
    let query = {
        let select_stmt = "\
            select entries.id, \
                   entries.uid, \
                   entries.journals_id, \
                   entries.users_id, \
                   entries.title, \
                   entries.entry_date, \
                   entries.created, \
                   entries.updated \
            from entries ";
        let mut where_parts = vec![String::from("entries.journals_id = $1")];
        //let mut offset_stmt = String::new();
        let order_parts = vec![String::from("entries.entry_date desc")];

        if start_date.is_some() && end_date.is_some() {
            if start_date > end_date {
                return Err(Error::Inner(SearchEntriesError::InvalidEndDate));
            }
        }

        if let Some(date) = &start_date {
            where_parts.push(format!(
                "entries.entry_date >= ${}",
                db::push_param(&mut params, date)
            ));
        }

        if let Some(date) = &end_date {
            where_parts.push(format!(
                "entries.entry_date <= ${}",
                db::push_param(&mut params, date)
            ));
        }

        /*
        if let Some(date) = &prev {
            match &dir {
                SearchDir::Frwd => where_parts.push(format!(
                    "entries.entry_date > ${}",
                    db::push_param(&mut params, date)
                )),
                SearchDir::Back => where_parts.push(format!(
                    "entries.entry_date <= ${}",
                    db::push_param(&mut params, date)
                )),
            }
        } else {
            write!(&mut offset_stmt, "offset {page}").unwrap();
        }
        */

        let where_stmt = where_parts.join(" and ");
        let order_stmt = order_parts.join(", ");

        //format!("{select_stmt} where {where_stmt} order by {order_stmt} {offset_stmt} limit {size}")
        format!("{select_stmt} where {where_stmt} order by {order_stmt}")
    };

    let start = std::time::Instant::now();

    let mut rtn = Vec::with_capacity((&size).into());

    if size > batch_size {
        let portal = conn.bind_raw(&query, params).await?;

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

                let (tags, custom_fields) = tokio::join!(
                    multi_query_tags(conn, &row.id),
                    multi_query_cfs(conn, &row.id)
                );

                rtn.push(EntryPartial::from((row, tags?, custom_fields?)));
            }

            if let Some(affected) = stream.get_ref().rows_affected() {
                if affected == 0 {
                    break;
                }
            }
        }
    } else {
        let stream = conn.query_raw(&query, params).await?.map(|result| {
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

            let (tags, custom_fields) = tokio::join!(
                multi_query_tags(conn, &row.id),
                multi_query_cfs(conn, &row.id)
            );

            rtn.push(EntryPartial::from((row, tags?, custom_fields?)));
        }
    }

    tracing::debug!("query time: {:#?}", start.elapsed());

    Ok(rtn)
}

async fn multi_query_tags(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
) -> Result<BTreeMap<String, Option<String>>, Error<SearchEntriesError>> {
    let params: db::ParamsArray<'_, 1> = [entries_id];

    Ok(conn
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
            result.map(|row| {
                (
                    row.get::<usize, String>(0),
                    row.get::<usize, Option<String>>(1),
                )
            })
        })
        .try_collect::<BTreeMap<String, Option<String>>>()
        .await?)
}

async fn multi_query_cfs(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
) -> Result<HashMap<CustomFieldId, custom_field::Value>, Error<SearchEntriesError>> {
    let params: db::ParamsArray<'_, 1> = [entries_id];
    Ok(conn
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
            result.map(|row| {
                (
                    row.get::<usize, CustomFieldId>(0),
                    row.get::<usize, custom_field::Value>(1),
                )
            })
        })
        .try_collect::<HashMap<CustomFieldId, custom_field::Value>>()
        .await?)
}
