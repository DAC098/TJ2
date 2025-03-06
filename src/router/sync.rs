use axum::Router;
use axum::extract::Path;
use axum::http::{StatusCode, Uri, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use chrono::{Utc, DateTime};
use futures::StreamExt;
use serde::{Serialize, Deserialize};

use crate::db;
use crate::db::ids::EntryId;
use crate::error::{self, Context};
use crate::router::body;
use crate::journal;
use crate::state;
use crate::sync;
use crate::sync::journal::SyncEntryResult;
use crate::user;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/entries", post(receive_entry))
}

async fn receive_entry(
    state: state::SharedState,
    body::Json(json): body::Json<sync::journal::EntrySync>,
) -> Result<SyncEntryResult, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    //tracing::debug!("received entry from server: {}", json.uid);

    let (journal_res, user_res) = tokio::join!(
        journal::Journal::retrieve(&transaction, &json.journals_uid),
        user::User::retrieve(&transaction, &json.users_uid),
    );

    let Some(journal) = journal_res.context("failed to retrieve journal")? else {
        tracing::debug!("failed to retrieve journal: {}", json.journals_uid);

        return Ok(SyncEntryResult::JournalNotFound);
    };

    let Some(user) = user_res.context("failed to retrieve_user")? else {
        tracing::debug!("failed to retrieve user: {}", json.users_uid);

        return Ok(SyncEntryResult::UserNotFound);
    };

    let result = transaction.query_one(
        "\
        insert into entries (uid, journals_id, users_id, entry_date, title, contents, created, updated) \
        values ($1, $2, $3, $4, $5, $6, $7, $8) \
        on conflict (uid) do update \
            set entry_date = excluded.entry_date, \
                title = excluded.title, \
                contents = excluded.contents, \
                updated = excluded.updated \
        returning id",
        &[&json.uid, &journal.id, &user.id, &json.date, &json.title, &json.contents, &json.created, &json.updated]
    )
        .await
        .context("failed to upsert entry")?;

    let entries_id: EntryId = result.get(0);

    upsert_tags(&transaction, &entries_id, &json.tags).await?;

    transaction.commit()
        .await
        .context("failed to commit entry sync transaction")?;

    Ok(SyncEntryResult::Synced)
}

async fn upsert_tags(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
    tags: &Vec<sync::journal::EntryTagSync>
) -> Result<(), error::Error> {
    if !tags.is_empty() {
        let mut params: db::ParamsVec<'_> = vec![entries_id];
        let mut query = String::from(
            "with tmp_insert as ( \
                insert into entry_tags (entries_id, key, value, created, updated) \
                values "
        );

        for (index, tag) in tags.iter().enumerate() {
            if index != 0 {
                query.push_str(", ");
            }

            let statement = format!(
                "($1, ${}, ${}, ${}, ${})",
                db::push_param(&mut params, &tag.key),
                db::push_param(&mut params, &tag.value),
                db::push_param(&mut params, &tag.created),
                db::push_param(&mut params, &tag.updated),
            );

            query.push_str(&statement);
        }

        query.push_str(" on conflict (entries_id, key) do update \
                set key = excluded.key, \
                    value = excluded.value, \
                    updated = excluded.updated \
                returning entries_id, key \
            ) \
            delete from entry_tags \
            using tmp_insert \
            where entry_tags.entries_id = tmp_insert.entries_id and \
                  entry_tags.key != tmp_insert.key"
        );

        conn.execute(&query, params.as_slice())
            .await
            .context("failed to upsert tags")?;
    } else {
        conn.execute(
            "\
            delete from entry_tags \
            where entries_id = $1",
            &[entries_id]
        )
            .await
            .context("failed to delete tags")?;
    }

    Ok(())
}
