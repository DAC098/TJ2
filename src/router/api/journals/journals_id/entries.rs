use std::collections::HashSet;
use std::default::Default;
use std::fmt::Write;

use chrono::Utc;
use futures::StreamExt;

use crate::db;
use crate::db::ids::{
    JournalId,
    EntryId,
    CustomFieldUid,
    FileEntryUid,
};
use crate::error::{self, Context};
use crate::fs::RemovedFiles;
use crate::router::body;
use crate::journal::{self, FileStatus, FileEntry};
use crate::sec::authn::ApiInitiator;
use crate::state;
use crate::sync;
use crate::sync::journal::{
    SyncEntryResult,
    EntryFileSync,
};

pub async fn post(
    state: state::SharedState,
    initiator: ApiInitiator,
    body::Json(json): body::Json<sync::journal::EntrySync>,
) -> Result<SyncEntryResult, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    tracing::debug!("received entry from server: {} {json:#?}", json.uid);

    let journal = {
        let Some(result) = journal::Journal::retrieve(&transaction, &json.journals_uid)
            .await
            .context("failed to retrieve journal")? else {
            tracing::debug!("failed to retrieve journal: {}", json.journals_uid);

            return Ok(SyncEntryResult::JournalNotFound);
        };

        result
    };

    if journal.users_id != initiator.user.id {
        return Ok(SyncEntryResult::JournalNotFound);
    }

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
        &[&json.uid, &journal.id, &initiator.user.id, &json.date, &json.title, &json.contents, &json.created, &json.updated]
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
        let journal_dir = state.storage()
            .journal_dir(journal.id);

        let UpsertFiles {
            not_found
        } = upsert_files(
            &transaction,
            &entries_id,
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
    journal_dir: journal::JournalDir,
    files: Vec<sync::journal::EntryFileSync>,
    removed_files: &mut RemovedFiles,
) -> Result<UpsertFiles, error::Error> {
    let status = FileStatus::Requested;
    let rtn = UpsertFiles::default();

    if !files.is_empty() {
        let mut known = FileEntry::retrieve_uid_map(conn, entries_id)
            .await
            .context("failed to retrieve known file entries")?;

        let created = Utc::now();

        let mut insert_id = HashSet::new();
        let mut ins_params: db::ParamsVec<'_> = vec![entries_id, &status, &created];
        let mut ins_query = String::from(
            "\
            insert into file_entries ( \
                entries_id, \
                status, \
                created, \
                uid, \
                name, \
                mime_type, \
                mime_subtype, \
                hash \
            ) \
            values "
        );

        let mut update_id = HashSet::new();
        let mut upd_params: db::ParamsVec<'_> = vec![&created];
        let mut upd_query = String::from(
            "\
            update file_entries \
            set name = tmp_update.name, \
                updated = $1 \
            from (values "
        );

        for (index, file) in files.iter().enumerate() {
            if let Some(exists) = known.get(&file.uid) {
                if !update_id.insert(exists.uid().clone()) {
                    // duplicate existing id
                    continue;
                }

                // we know that the file exists for this entry so we will not 
                // need to check the entry id
                if update_id.len() > 1 {
                    upd_query.push_str(", ");
                }

                write!(
                    &mut upd_query,
                    "(${}, ${})",
                    db::push_param(&mut upd_params, exists.id_ref()),
                    db::push_param(&mut upd_params, &file.name),
                ).unwrap();
            } else {
                // do a lookup to make sure that the uid exists for a different
                // entry
                let lookup_result = FileEntry::retrieve(conn, &file.uid)
                    .await
                    .context("failed to lookup file uid")?
                    .is_some_and(|v| v.entries_id() == *entries_id);

                if lookup_result {
                    // the given uid exists but is not attached to the
                    // entry we are currently working on
                    continue;
                }

                if !insert_id.insert(file.uid.clone()) {
                    // duplicate inserting uid
                    continue;
                }

                if insert_id.len() > 1 {
                    ins_query.push_str(", ");
                }

                write!(
                    &mut ins_query,
                    "($1, $2, $3, ${}, ${}, '', '', '')",
                    db::push_param(&mut ins_params, &file.uid),
                    db::push_param(&mut ins_params, &file.name),
                ).unwrap();
            }
        }

        let to_drop: Vec<FileEntryUid> = known.keys()
            .filter(|v| !update_id.contains(v))
            .map(|v| v.clone())
            .collect();

        if !to_drop.is_empty() {
            // we will delete the entries that were not found in order to
            // prevent the possibility that a new record or an updated one has
            // a similar name
            tracing::debug!("deleting file entries: {}", to_drop.len());

            conn.execute(
                "delete from file_entries where uid = any($1)",
                &[&to_drop]
            )
                .await
                .context("failed to delete from file entries")?;
        }

        if !insert_id.is_empty() {
            tracing::debug!("inserting file entries: {} {ins_query}", insert_id.len());

            conn.execute(&ins_query, ins_params.as_slice())
                .await
                .context("failed to insert files")?;
        }

        if !update_id.is_empty() {
            tracing::debug!("updating file entries: {} {upd_query}", update_id.len());

            conn.execute(&upd_query, upd_params.as_slice())
                .await
                .context("failed to update files")?;
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
                FileEntry::Requested(_) => {}
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
