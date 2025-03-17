use std::default::Default;

use axum::Router;
use axum::routing::post;
use futures::StreamExt;

use crate::db;
use crate::db::ids::{
    JournalId,
    EntryId,
    CustomFieldUid,
    FileEntryUid,
    RemoteServerId,
};
use crate::error::{self, Context};
use crate::fs::RemovedFiles;
use crate::router::body;
use crate::journal::{self, FileStatus, FileEntry};
use crate::state;
use crate::sync;
use crate::sync::journal::{
    SyncEntryResult,
    EntryFileSync,
};
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

    tracing::debug!("received entry from server: {} {json:#?}", json.uid);

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

    {
        let UpsertCFS {
            not_found,
            invalid
        } = upsert_cfs(&transaction, &journal.id, &entries_id, &json.custom_fields).await?;

        if !not_found.is_empty() {
            return Ok(SyncEntryResult::CFNotFound {
                uids: not_found
            });
        }

        if !invalid.is_empty() {
            return Ok(SyncEntryResult::CFInvalid {
                uids: invalid
            });
        }
    }

    let mut removed_files = RemovedFiles::new();

    {
        let tmp_server_id = RemoteServerId::new(1).unwrap();
        let journal_dir = state.storage()
            .journal_dir(&journal);

        let UpsertFiles {
            not_found
        } = upsert_files(
            &transaction,
            &entries_id,
            &tmp_server_id,
            journal_dir,
            json.files,
            &mut removed_files
        ).await?;

        if !not_found.is_empty() {
            return Ok(SyncEntryResult::FileNotFound {
                uids: not_found,
            });
        }
    }

    let result = transaction.commit()
        .await
        .context("failed to commit entry sync transaction");

    if let Err(err) = result {
        removed_files.log_rollback().await;

        return Err(err);
    }

    removed_files.log_clean().await;

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

struct UpsertCFS {
    not_found: Vec<CustomFieldUid>,
    invalid: Vec<CustomFieldUid>,
}

impl Default for UpsertCFS {
    fn default() -> Self {
        UpsertCFS {
            not_found: Vec::new(),
            invalid: Vec::new(),
        }
    }
}

async fn upsert_cfs(
    conn: &impl db::GenericClient,
    journals_id: &JournalId,
    entries_id: &EntryId,
    cfs: &Vec<sync::journal::EntryCFSync>,
) -> Result<UpsertCFS, error::Error> {
    let mut results = UpsertCFS::default();

    if !cfs.is_empty() {
        let fields = journal::CustomField::retrieve_journal_uid_map(conn, journals_id)
            .await
            .context("failed to retrieve journal custom fields")?;
        let mut counted = 0;
        let mut params: db::ParamsVec<'_> = vec![entries_id];
        let mut query = String::from(
            "\
            with tmp_insert as ( \
                insert into custom_field_entries (entries_id, custom_fields_id, value, created, updated) \
                values "
        );

        for (index, cf) in cfs.iter().enumerate() {
            let Some(field_ref) = fields.get(&cf.custom_fields_uid) else {
                results.not_found.push(cf.custom_fields_uid.clone());

                continue;
            };

            if index != 0 {
                query.push_str(", ");
            }

            let statement = format!(
                "($1, ${}, ${}, ${}, ${})",
                db::push_param(&mut params, &field_ref.id),
                db::push_param(&mut params, &cf.value),
                db::push_param(&mut params, &cf.created),
                db::push_param(&mut params, &cf.updated),
            );

            query.push_str(&statement);

            counted += 1;
        }

        if counted > 0 {
            query.push_str(" on conflict (custom_fields_id, entries_id) do update \
                    set value = excluded.value, \
                        updated = excluded.updated \
                    returning custom_fields_id, \
                              entries_id \
                ) \
                delete from custom_field_entries \
                using tmp_insert \
                where custom_field_entries.entries_id = tmp_insert.entries_id and \
                      custom_field_entries.custom_fields_id != tmp_insert.custom_fields_id"
            );

            //tracing::debug!("query: {query}");

            conn.execute(&query, params.as_slice())
                .await
                .context("failed to upsert custom fields")?;
        }
    } else {
        conn.execute(
            "\
            delete from custom_field_entries \
            where entries_id = $1",
            &[entries_id]
        )
            .await
            .context("failed to delete custom fields")?;
    }

    Ok(results)
}

#[derive(Debug, Default)]
struct UpsertFiles {
    not_found: Vec<FileEntryUid>
}

async fn upsert_files(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
    server_id: &RemoteServerId,
    journal_dir: journal::JournalDir,
    files: Vec<sync::journal::EntryFileSync>,
    removed_files: &mut RemovedFiles,
) -> Result<UpsertFiles, error::Error> {
    let status = FileStatus::Remote;
    let mut rtn = UpsertFiles::default();

    if !files.is_empty() {
        let mut known = FileEntry::retrieve_uid_map(conn, entries_id)
            .await
            .context("failed to retrieve known file entries")?;
        let mut counted = 0;
        let mut params: db::ParamsVec<'_> = vec![entries_id, &status, server_id];
        let mut query = String::from(
            "\
            insert into file_entries ( \
                entries_id, \
                status, \
                server_id, \
                uid, \
                name, \
                mime_type, \
                mime_subtype, \
                mime_param, \
                size, \
                hash, \
                created, \
                updated \
            ) \
            values "
        );

        for (index, file) in files.iter().enumerate() {
            if let Some(exists) = known.remove(file.uid()) {
                // we know that the file exists for this entry so we will not 
                // need to check the entry id
                match exists {
                    FileEntry::Requested(_) => {
                        // the journal should not have these as the peer should
                        // only send received files and the local server should
                        // not be modifying the journal
                        return Err(error::Error::context(
                            "encountered requested file when removing local files"
                        ));
                    }
                    // skip the other entries as we do not need to worry about
                    // them
                    _ => {}
                }
            } else {
                // do a lookup to make sure that the uid exists for a different
                // entry
                let lookup_result = FileEntry::retrieve(conn, file.uid())
                    .await
                    .context("failed to lookup file uid")?;

                if let Some(found) = lookup_result {
                    match found {
                        FileEntry::Received(rec) => if rec.entries_id != *entries_id {
                            // the given uid exists but is not attached to the
                            // entry we are currently working on
                            continue;
                        },
                        FileEntry::Remote(rmt) => if rmt.entries_id != *entries_id {
                            // same as received
                            continue;
                        },
                        FileEntry::Requested(_) => {
                            return Err(error::Error::context(
                                "encountered requested file when removing local files"
                            ));
                        }
                    }
                }
            }
            if index != 0 {
                query.push_str(", ");
            }

            match file {
                EntryFileSync::Received(rec) => {
                    let statement = format!(
                        "($1, $2, $3, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                        db::push_param(&mut params, &rec.uid),
                        db::push_param(&mut params, &rec.name),
                        db::push_param(&mut params, &rec.mime_type),
                        db::push_param(&mut params, &rec.mime_subtype),
                        db::push_param(&mut params, &rec.mime_param),
                        db::push_param(&mut params, &rec.size),
                        db::push_param(&mut params, &rec.hash),
                        db::push_param(&mut params, &rec.created),
                        db::push_param(&mut params, &rec.updated),
                    );

                    query.push_str(&statement);
                }
            }

            counted += 1;
        }

        if !known.is_empty() {
            // we will delete the entries that were not found in order to
            // prevent the posibility that a new record or an updated one has
            // a similar name
            let uids = known.into_keys()
                .collect::<Vec<FileEntryUid>>();

            tracing::debug!("deleting file entries: {}", uids.len());

            conn.execute(
                "delete from file_entries where uid = any($1)",
                &[&uids]
            )
                .await
                .context("failed to delete from file entries")?;
        }

        if counted > 0 {
            query.push_str(" on conflict (uid) do update \
                set name = excluded.name, \
                    updated = excluded.updated \
                returning id, \
                          entries_id"
            );

            tracing::debug!("upserting file entries: {counted} {query}");

            conn.execute(&query, params.as_slice())
                .await
                .context("failed to upsert files")?;
        }
    } else {
        // delete all files that are local to the manchine and just remove the
        // entries that are marked remote
        let known_files = FileEntry::retrieve_entry_stream(conn, entries_id)
            .await
            .context("failed to retrieve file entries")?;
        let mut ids = Vec::new();

        futures::pin_mut!(known_files);

        while let Some(try_record) = known_files.next().await {
            let file = try_record.context("failed to retrieve record")?;

            match file {
                FileEntry::Received(rec) => {
                    removed_files.add(journal_dir.file_path(&rec.id))
                        .await
                        .context("failed to remove received journal file")?;

                    ids.push(rec.id);
                }
                FileEntry::Remote(rmt) => {
                    ids.push(rmt.id);
                }
                FileEntry::Requested(_) => {
                    return Err(error::Error::context(
                        "encountered requested file when removing local files"
                    ));
                }
            }
        }

        conn.execute(
            "delete from file_entries where entries_id = $1",
            &[entries_id]
        )
            .await
            .context("failed to delete file entries")?;
    }

    Ok(rtn)
}
