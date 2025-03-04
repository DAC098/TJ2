use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use futures::stream::FuturesUnordered;
use reqwest::StatusCode;

use crate::db::{
    ParamsArray,
    ParamsVec,
    GenericClient,
    push_param,
};
use crate::db::ids::{
    JournalId,
    EntryId,
    RemoteServerId,
};
use crate::error::{self, Context};
use crate::journal::Journal;
use crate::state;
use crate::sync::{self, RemoteServer, RemoteClient};

const BATCH_SIZE: i32 = 50;

pub async fn sync_journal(
    state: state::SharedState,
    remote: RemoteServer,
    journal: Journal,
) {
    let mut conn = match state.db_conn().await {
        Ok(conn) => conn,
        Err(err) => {
            error::log_prefix_error("failed to get database connection", &err);

            return;
        }
    };
    let mut transaction = match conn.transaction().await {
        Ok(trans) => trans,
        Err(err) => {
            error::log_prefix_error("failed to create transaction", &err);

            return;
        }
    };

    let client = match RemoteClient::build(remote) {
        Ok(client) => client,
        Err(err) => {
            error::log_prefix_error("failed to create remote client", &err);

            return;
        }
    };

    let mut prev_entry = EntryId::zero();
    let sync_date = Utc::now();

    loop {
        let checkpoint = match transaction.transaction().await {
            Ok(check) => check,
            Err(err) => {
                error::log_prefix_error(
                    "failed to create savepoint for journal sync",
                    &err
                );

                break;
            }
        };

        let result = batch_sync(
            &checkpoint,
            &client,
            &journal,
            &prev_entry,
            &sync_date
        ).await;

        let (counted, last_id) = match result {
            Ok(rtn) => {
                if let Err(err) = checkpoint.commit().await {
                    error::log_prefix_error(
                        "failed to commit batch entries savepoint",
                        &err
                    );
                }

                rtn
            }
            Err(err) => {
                error::log_prefix_error(
                    "failed to sync batch entries",
                    &err
                );

                if let Err(err) = checkpoint.rollback().await {
                    error::log_prefix_error(
                        "failed to rollback batch entries savepoint",
                        &err
                    );
                }

                break;
            }
        };

        if counted != (BATCH_SIZE as usize) {
            break;
        } else {
            prev_entry = last_id;
        }
    }

    // we are going to try and commit the changes in the main transaction as
    // the only thing that should have been updated is the synced_entries
    // table and we want to try and avoid send data that we have already
    // sent to the remote server

    if let Err(err) = transaction.commit().await {
        error::log_prefix_error(
            "failed to commit top transaction for sync journal",
            &err
        );
    }
}

async fn batch_sync(
    conn: &impl GenericClient,
    client: &RemoteClient,
    journal: &Journal,
    prev_entry: &EntryId,
    sync_date: &DateTime<Utc>,
) -> Result<(usize, EntryId), error::Error> {
    let entries = query_entries(
        conn,
        &journal.id,
        client.remote().id(),
        prev_entry,
        sync_date,
    ).await?;

    futures::pin_mut!(entries);

    let mut futs = FuturesUnordered::new();
    let mut last_id = *prev_entry;
    let mut counted = 0;

    while let Some(try_record) = entries.next().await {
        let (entries_id, mut entry) = try_record?;

        let (tags_res, custom_fields_res, files_res) = tokio::join!(
            query_tags(conn, &entries_id),
            query_custom_fields(conn, &entries_id),
            query_files(conn, &entries_id),
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
        successful,
        client.remote().id(),
        sync::journal::SyncStatus::Synced,
        sync_date
    ).await?;
    update_synced(
        conn,
        failed,
        client.remote().id(),
        sync::journal::SyncStatus::Failed,
        sync_date
    ).await?;

    Ok((counted, last_id))
}

async fn query_entries(
    conn: &impl GenericClient,
    journals_id: &JournalId,
    server_id: &RemoteServerId,
    prev_entry: &EntryId,
    sync_date: &DateTime<Utc>,
) -> Result<impl Stream<Item = Result<(EntryId, sync::journal::EntrySync), error::Error>>, error::Error> {
    let params: ParamsArray<5> = [journals_id, server_id, prev_entry, sync_date, &BATCH_SIZE];
    let stream = conn.query_raw(
        "\
        select entries.id, \
               entries.uid, \
               journals.uid, \
               users.uid, \
               entries.entry_date, \
               entries.title, \
               entries.contents, \
               entries.created, \
               entries.updated \
        from entries \
            left join users on \
                entries.users_id = users.id \
            left join journals on \
                entries.journals_id = journals.id \
            left join synced_entries on \
                entries.id = synced_entries.entries_id and \
                synced_entries.server_id = $2 \
        where entries.journals_id = $1 and \
              entries.entries_id > $3 and ( \
                  synced_entries.status is null or ( \
                      synced_entries.updated < ( \
                          case when entries.updated is null \
                              then entries.created \
                              else entries.updated \
                      ) and \
                      synced_entries.updated < $4 \
                  ) \
              ) \
        order by entries.id \
        limit $5",
        params
    )
        .await
        .context("failed to retrieve entries batch")?;

    Ok(stream.map(|try_record| match try_record {
        Ok(record) => Ok((record.get(0), sync::journal::EntrySync {
            uid: record.get(1),
            journals_uid: record.get(2),
            users_uid: record.get(3),
            date: record.get(4),
            title: record.get(5),
            contents: record.get(6),
            created: record.get(7),
            updated: record.get(8),
            tags: Vec::new(),
            custom_fields: Vec::new(),
            files: Vec::new()
        })),
        Err(err) => Err(error::Error::context_source(
            "failed to retrieve entry record",
            err
        ))
    }))
}

async fn query_tags(
    conn: &impl GenericClient,
    entries_id: &EntryId
) -> Result<Vec<sync::journal::EntryTagSync>, error::Error> {
    let params: ParamsArray<1> = [entries_id];
    let stream = conn.query_raw(
        "\
        select entry_tags.key, \
               entry_tags.value, \
               entry_tags.created, \
               entry_tags.updated \
        from entry_tags \
        where entry_tags.entries_id = $1",
        params,
    )
        .await
        .context("failed to retrieve entry tags")?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let record = try_record.context("failed to retrieve entry tag record")?;

        rtn.push(sync::journal::EntryTagSync {
            key: record.get(0),
            value: record.get(1),
            created: record.get(2),
            updated: record.get(3),
        });
    }

    Ok(rtn)
}

async fn query_custom_fields(
    conn: &impl GenericClient,
    entries_id: &EntryId
) -> Result<Vec<sync::journal::EntryCFSync>, error::Error> {
    let params: ParamsArray<1> = [entries_id];
    let stream = conn.query_raw(
        "\
        select custom_fields.uid, \
               custom_field_entries.value, \
               custom_field_entries.created, \
               custom_field_entries.updated \
        from custom_field_entries \
            left join custom_fields on \
                custom_field_entries.custom_fields_id = custom_fields.id \
        where custom_field_entries.entries_id = $1",
        params,
    )
        .await
        .context("failed to retrieve entry custom fields")?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let record = try_record.context("failed to retrieve entry custom field record")?;

        rtn.push(sync::journal::EntryCFSync {
            custom_fields_uid: record.get(0),
            value: record.get(1),
            created: record.get(2),
            updated: record.get(3),
        });
    }

    Ok(rtn)
}

async fn query_files(
    conn: &impl GenericClient,
    entries_id: &EntryId
) -> Result<Vec<sync::journal::EntryFileSync>, error::Error> {
    let params: ParamsArray<1> = [entries_id];
    let stream = conn.query_raw(
        "\
        select file_entries.uid, \
               file_entries.name, \
               file_entries.mime_type, \
               file_entries.mime_subtype, \
               file_entries.mime_param, \
               file_entries.size, \
               file_entries.created, \
               file_entries.updated \
        from file_entries \
        where file_entries.id = $1",
        params
    )
        .await
        .context("failed to retrieve entry files")?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let record = try_record.context("failed to retrieve entry file record")?;

        rtn.push(sync::journal::EntryFileSync {
            uid: record.get(0),
            name: record.get(1),
            mime_type: record.get(2),
            mime_subtype: record.get(3),
            mime_param: record.get(4),
            size: record.get(5),
            created: record.get(6),
            updated: record.get(7),
        });
    }

    Ok(rtn)
}

async fn update_synced(
    conn: &impl GenericClient,
    given: Vec<EntryId>,
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
        StatusCode::ACCEPTED => Ok(entries_id),
        StatusCode::INTERNAL_SERVER_ERROR => Err(entries_id),
        _ => Err(entries_id),
    }
}
