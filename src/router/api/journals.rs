use axum::http::StatusCode;
use chrono::Utc;
use futures::StreamExt;

use crate::journal::{CustomField, Journal, CustomFieldBuilder};
use crate::net::{Error, body};
use crate::sec::authn::ApiInitiator;
use crate::state;
use crate::sync;

pub mod journals_id;

pub async fn post(
    state: state::SharedState,
    initiator: ApiInitiator,
    body::Json(json): body::Json<sync::journal::JournalSync>,
) -> Result<StatusCode, Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction().await?;

    let now = Utc::now();

    let journal = if let Some(mut exists) = Journal::retrieve(&transaction, &json.uid).await?
    {
        if exists.users_id != initiator.user.id {
            return Ok(StatusCode::BAD_REQUEST);
        }

        exists.updated = Some(now);
        exists.name = json.name;
        exists.description = json.description;

        if let Err(err) = exists.update(&transaction).await {
            return Err(Error::source(err));
        }

        exists
    } else {
        let mut options = Journal::create_options(initiator.user.id, json.name);
        options.uid(json.uid);

        if let Some(desc) = json.description {
            options.description(desc);
        }

        let journal = match Journal::create(&transaction, options).await {
            Ok(journal) => journal,
            Err(err) => return Err(Error::source(err)),
        };

        journal
    };

    let mut builders = Vec::new();

    for cf in json.custom_fields {
        let mut builder = CustomField::builder(journal.id, cf.name, cf.config);
        builder.with_uid(cf.uid);
        builder.with_order(cf.order);

        if let Some(desc) = cf.description {
            builder.with_description(desc);
        }

        builders.push(builder);
    }

    if let Some(stream) = CustomFieldBuilder::build_many(&transaction, builders).await? {
        futures::pin_mut!(stream);

        while let Some(result) = stream.next().await {
            result?;
        }
    }

    transaction.commit().await?;

    Ok(StatusCode::CREATED)
}
