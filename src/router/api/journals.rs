use axum::http::StatusCode;
use chrono::Utc;

use crate::error::{self, Context};
use crate::journal::{CustomField, Journal};
use crate::router::body;
use crate::sec::authn::ApiInitiator;
use crate::state;
use crate::sync;
use crate::sync::journal::{EntryFileSync, SyncEntryResult};

pub mod journals_id;

pub async fn post(
    state: state::SharedState,
    initiator: ApiInitiator,
    body::Json(json): body::Json<sync::journal::JournalSync>,
) -> Result<StatusCode, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn
        .transaction()
        .await
        .context("failed to create transaction")?;

    let now = Utc::now();

    let journal = if let Some(mut exists) = Journal::retrieve(&transaction, &json.uid)
        .await
        .context("failed to retrieve journal")?
    {
        if exists.users_id != initiator.user.id {
            return Ok(StatusCode::BAD_REQUEST);
        }

        exists.updated = Some(now);
        exists.name = json.name;
        exists.description = json.description;

        exists
            .update(&transaction)
            .await
            .context("failed to update journal")?;

        exists
    } else {
        let mut options = Journal::create_options(initiator.user.id, json.name);
        options.uid(json.uid);

        if let Some(desc) = json.description {
            options.description(desc);
        }

        let journal = Journal::create(&transaction, options)
            .await
            .context("failed to create journal")?;

        journal
    };

    for cf in json.custom_fields {
        let mut options = CustomField::create_options(journal.id, cf.name, cf.config);
        options.uid = Some(cf.uid);
        options.order = cf.order;
        options.description = cf.description;

        CustomField::create(&transaction, options)
            .await
            .context("failed to create custom field")?;
    }

    transaction
        .commit()
        .await
        .context("failed to commit transaction")?;

    Ok(StatusCode::CREATED)
}
