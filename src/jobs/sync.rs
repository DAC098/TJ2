use chrono::{DateTime, Utc};
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use reqwest::StatusCode;

use crate::db::{
    ParamsVec,
    GenericClient,
    push_param,
};
use crate::db::ids::{
    EntryId,
    RemoteServerId,
};
use crate::error::{self, Context};
use crate::journal::Journal;
use crate::state;
use crate::sync::{self, RemoteServer, RemoteClient};

const BATCH_SIZE: i64 = 50;

pub async fn kickoff_sync_journal(state: state::SharedState, remote: RemoteServer, journal: Journal) {
    if let Err(err) = sync_journal(state, remote, journal).await {
        error::log_prefix_error("error when sync journal with remote server", &err);
    }
}

async fn sync_journal(
    state: state::SharedState,
    remote: RemoteServer,
    journal: Journal,
) -> Result<(), error::Error> {
    let mut conn = state.db_conn()
        .await
        .context("failed to get database connection")?;
    let mut transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let client = RemoteClient::build(remote)
        .context("failed to create remote client")?;

    let max_batches = 2;
    let mut batches = 0;
    let mut total_successful = 0;
    let mut total_failed = 0;
    let mut prev_entry = EntryId::zero();
    let sync_date = Utc::now();

    loop {
        let Some(checkpoint) = error::prefix_try_result(
            "failed to create savepoint ofr journal sync",
            transaction.transaction().await
        ) else {
            break;
        };

        let result = {
            let batch_result = batch_entry_sync(
                &checkpoint,
                &client,
                &journal,
                &prev_entry,
                &sync_date
            ).await;

            match batch_result {
                Ok(rtn) => {
                    if let Err(err) = checkpoint.commit().await {
                        error::log_prefix_error("failed to commit batch entries savepoint", &err);
                    }

                    rtn
                }
                Err(err) => {
                    error::log_prefix_error("failed to sync batch entries", &err);

                    if let Err(err) = checkpoint.rollback().await {
                        error::log_prefix_error("failed to rollback batch entries savepoint", &err);
                    }

                    break;
                }
            }
        };

        tracing::debug!("batch results: {result:#?}");

        let BatchResults {
            last_id,
            counted,
            successful,
            failed,
        } = result;

        total_successful += successful.len();
        total_failed += failed.len();

        if counted != BATCH_SIZE {
            break;
        } else {
            prev_entry = last_id;
        }

        batches += 1;

        if batches == max_batches {
            break;
        }
    }

    tracing::debug!("batch sync complete. successful: {total_successful} failed: {total_failed}");

    // we are going to try and commit the changes in the main transaction as
    // the only thing that should have been updated is the synced_entries
    // table and we want to try and avoid send data that we have already
    // sent to the remote server

    transaction.commit()
        .await
        .context("failed to commit top transaction for sync journal")?;

    Ok(())
}

#[derive(Debug)]
struct BatchResults {
    last_id: EntryId,
    counted: i64,
    successful: Vec<EntryId>,
    failed: Vec<EntryId>,
}

async fn batch_entry_sync(
    conn: &impl GenericClient,
    client: &RemoteClient,
    journal: &Journal,
    prev_entry: &EntryId,
    sync_date: &DateTime<Utc>,
) -> Result<BatchResults, error::Error> {
    let entries = sync::journal::EntrySync::retrieve_batch_stream(
        conn,
        &journal.id,
        client.remote().id(),
        prev_entry,
        sync_date,
        BATCH_SIZE
    ).await?;

    futures::pin_mut!(entries);

    let mut futs = FuturesUnordered::new();
    let mut last_id = *prev_entry;
    let mut counted = 0;

    while let Some(try_record) = entries.next().await {
        let (entries_id, mut entry) = try_record?;

        let (tags_res, custom_fields_res, files_res) = tokio::join!(
            sync::journal::EntryTagSync::retrieve(conn, &entries_id),
            sync::journal::EntryCFSync::retrieve(conn, &entries_id),
            sync::journal::EntryFileSync::retrieve(conn, &entries_id),
        );

        entry.tags = tags_res?;
        entry.custom_fields = custom_fields_res?;
        entry.files = files_res?;

        futs.push(send_entry(client, entries_id, entry));

        last_id = entries_id;
        counted += 1;
    }

    let mut successful = Vec::new();
    let mut failed = Vec::new();

    while let Some(result) = futs.next().await {
        match result {
            Ok(id) => successful.push(id),
            Err(id) => failed.push(id),
        }
    }

    update_synced(
        conn,
        &successful,
        client.remote().id(),
        sync::journal::SyncStatus::Synced,
        sync_date
    ).await?;
    update_synced(
        conn,
        &failed,
        client.remote().id(),
        sync::journal::SyncStatus::Failed,
        sync_date
    ).await?;

    Ok(BatchResults {
        last_id,
        counted,
        successful,
        failed,
    })
}

async fn update_synced(
    conn: &impl GenericClient,
    given: &Vec<EntryId>,
    server_id: &RemoteServerId,
    status: sync::journal::SyncStatus,
    updated: &DateTime<Utc>,
) -> Result<(), error::Error> {
    if given.is_empty() {
        return Ok(());
    }

    let mut params: ParamsVec<'_> = vec![server_id, &status, updated];
    let mut query = String::from(
        "insert into synced_entries (entries_id, server_id, status, updated) values "
    );

    for (index, entries_id) in given.iter().enumerate() {
        if index > 0 {
            query.push_str(", ");
        }

        let statement = format!(
            "(${}, $1, $2, $3)",
            push_param(&mut params, entries_id)
        );

        query.push_str(&statement);
    }

    query.push_str(
        " on conflict (entries_id, server_id) do update \
        set status = excluded.status, \
            updated = excluded.updated"
    );

    conn.execute(&query, params.as_slice())
        .await
        .context("failed to update synced entries")?;

    Ok(())
}

async fn send_entry(
    client: &RemoteClient,
    entries_id: EntryId,
    entry: sync::journal::EntrySync,
) -> Result<EntryId, EntryId> {
    let result = client.post("/sync/entries")
        .json(&entry)
        .send()
        .await;

    let Some(res) = error::prefix_try_result("failed to send entry to remote server", result) else {
        return Err(entries_id);
    };

    match res.status() {
        StatusCode::CREATED => Ok(entries_id),
        StatusCode::INTERNAL_SERVER_ERROR => Err(entries_id),
        _ => Err(entries_id),
    }
}
