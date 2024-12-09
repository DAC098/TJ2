use axum::Router;
use axum::extract::Path;
use axum::http::{StatusCode, Uri, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use chrono::{Utc, DateTime};
use futures::StreamExt;
use serde::{Serialize, Deserialize};

use crate::state;
use crate::db;
use crate::db::ids::{JournalId, JournalUid, UserId};
use crate::error::{self, Context};
use crate::journal::Journal;
use crate::router::body;
use crate::router::macros;
use crate::sec::authz::{self, Scope, Ability};

mod entries;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/", get(retrieve_journals)
            .post(create_journal))
        .route("/new", get(retrieve_journal))
        .route("/:journals_id", get(retrieve_journal))
        .route("/:journals_id/entries", get(entries::retrieve_entries)
            .post(entries::create_entry))
        .route("/:journals_id/entries/new", get(entries::retrieve_entry))
        .route("/:journals_id/entries/:entries_id", get(entries::retrieve_entry)
            .patch(entries::update_entry)
            .delete(entries::delete_entry))
        .route("/:journals_id/entries/:entries_id/:file_entry_id", get(entries::files::retrieve_file)
            .put(entries::files::upload_file))
}

#[derive(Debug, Serialize)]
pub struct JournalPartial {
    pub id: JournalId,
    pub uid: JournalUid,
    pub users_id: UserId,
    pub name: String,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

async fn retrieve_journals(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator!(
        &conn,
        &headers,
        Some(uri.clone())
    );

    macros::res_if_html!(state.templates(), &headers);

    let perm_check = authz::has_permission(
        &conn,
        initiator.user.id,
        Scope::Journals,
        Ability::Read
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let params: db::ParamsArray<'_, 1> = [&initiator.user.id];
    let journals = conn.query_raw(
        "\
        with search_journals as ( \
            select * \
            from journals \
            where journals.users_id = $1 \
        ) \
        select search_journals.id, \
               search_journals.uid, \
               search_journals.users_id, \
               search_journals.name, \
               search_journals.created, \
               search_journals.updated \
        from search_journals \
        order by search_journals.name",
        params
    )
        .await
        .context("failed to retrieve journals")?;

    futures::pin_mut!(journals);

    let mut found = Vec::new();

    while let Some(try_record) = journals.next().await {
        let record = try_record.context("failed to retrieve journal")?;

        found.push(JournalPartial {
            id: record.get(0),
            uid: record.get(1),
            users_id: record.get(2),
            name: record.get(3),
            created: record.get(4),
            updated: record.get(5),
        });
    }

    Ok(body::Json(found).into_response())
}

#[derive(Debug, Deserialize)]
pub struct MaybeJournalPath {
    journals_id: Option<JournalId>,
}

#[derive(Debug, Deserialize)]
pub struct JournalPath {
    journals_id: JournalId
}

#[derive(Debug, Serialize)]
pub struct JournalFull {
    pub id: JournalId,
    pub uid: JournalUid,
    pub users_id: UserId,
    pub name: String,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

async fn retrieve_journal(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
    Path(MaybeJournalPath { journals_id }): Path<MaybeJournalPath>,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let Some(journals_id) = journals_id else {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator!(&conn, &headers, Some(uri));

    let perm_check = authz::has_permission(
        &conn,
        initiator.user.id,
        Scope::Journals,
        Ability::Read
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = Journal::retrieve_id(&conn, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    Ok(body::Json(JournalFull {
        id: journal.id,
        uid: journal.uid,
        users_id: journal.users_id,
        name: journal.name,
        created: journal.created,
        updated: journal.updated,
    }).into_response())
}

#[derive(Debug, Deserialize)]
pub struct NewJournal {
    name: String
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum NewJournalResult {
    NameExists,
    Created(JournalFull)
}

async fn create_journal(
    state: state::SharedState,
    headers: HeaderMap,
    body::Json(json): body::Json<NewJournal>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator!(&transaction, &headers, None::<Uri>);

    let perm_check = authz::has_permission(
        &transaction,
        initiator.user.id,
        Scope::Journals,
        Ability::Create
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = Journal::create(&transaction, initiator.user.id, &json.name)
        .await
        .context("failed to create new journal")?;

    let Some(journal) = result else {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewJournalResult::NameExists)
        ).into_response());
    };

    let journal_dir= state.storage()
        .journal_dir(&journal);

    let root_dir = journal_dir.create_root_dir()
        .await
        .context("failed to create root journal directory")?;

    let files_dir = match journal_dir.create_files_dir().await {
        Ok(files) => files,
        Err(err) => {
            if let Err(root_err) = tokio::fs::remove_dir(&root_dir).await {
                error::log_prefix_error(
                    "failed to remove journal root dir",
                    &root_err
                );
            }

            return Err(error::Error::context_source("failed to create journal files dir", err));
        }
    };

    if let Err(err) = transaction.commit().await {
        if let Err(files_err) = tokio::fs::remove_dir(&files_dir).await {
            error::log_prefix_error(
                "failed to remove journal files dir",
                &files_err
            );
        } else if let Err(root_err) = tokio::fs::remove_dir(&root_dir).await {
            error::log_prefix_error(
                "failed to remove journal root dir",
                &root_err
            );
        }

        return Err(error::Error::context_source(
            "failed to commit transaction",
            err
        ));
    }

    Ok(body::Json(NewJournalResult::Created(JournalFull {
        id: journal.id,
        uid: journal.uid,
        users_id: journal.users_id,
        name: journal.name,
        created: journal.created,
        updated: journal.updated,
    })).into_response())
}
