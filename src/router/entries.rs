use std::collections::HashMap;
use std::fmt::Write;

use axum::extract::{Path, Request};
use axum::http::{StatusCode, Uri, HeaderMap};
use axum::response::{IntoResponse, Response};
use chrono::{NaiveDate, Utc, DateTime};
use futures::StreamExt;
use serde::{Serialize, Deserialize};
use sqlx::{QueryBuilder, Row, Execute};

use crate::state;
use crate::db;
use crate::db::ids::{EntryId, EntryUid, FileEntryId, FileEntryUid, JournalId, UserId};
use crate::error::{self, Context};
use crate::journal::{Journal, EntryTag, Entry, EntryFull, FileEntry};
use crate::router::body;
use crate::router::macros;
use crate::sec::authz::{Scope, Ability};

mod auth;

pub mod files;

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
    pub tags: HashMap<String, Option<String>>,
}

pub async fn retrieve_entries(
    state: state::SharedState,
    req: Request,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), req.headers());

    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator_pg!(
        &conn,
        req.headers(),
        Some(req.uri().clone())
    );

    let result = Journal::retrieve_default_pg(&conn, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    auth::perm_check_pg!(&conn, initiator, journal, Scope::Entries, Ability::Read);

    let params: db::ParamsArray<'_, 2> = [&initiator.user.id, &journal.id];
    let entries = conn.query_raw(
        "\
        with search_entries as ( \
            select * \
            from entries \
            where entries.users_id = $1 and \
                  entries.journals_id = $2 \
        ) \
        select search_entries.id, \
               search_entries.uid, \
               search_entries.journals_id, \
               search_entries.users_id, \
               search_entries.title, \
               search_entries.entry_date, \
               search_entries.created, \
               search_entries.updated, \
               entry_tags.key, \
               entry_tags.value
        from search_entries \
            left join entry_tags on \
                search_entries.id = entry_tags.entries_id \
        order by search_entries.entry_date desc",
        params
    )
        .await
        .context("failed to retrieve journal entries")?;

    futures::pin_mut!(entries);

    let mut found = Vec::new();
    let mut current: Option<EntryPartial> = None;

    while let Some(try_record) = entries.next().await {
        let record = try_record.context("failed to retrieve journal entry")?;
        let key: Option<String> = record.get(8);
        let value: Option<String> = record.get(9);

        if let Some(curr) = &mut current {
            let id = record.get(0);

            if curr.id == id {
                if let Some(key) = key {
                    curr.tags.insert(key, value);
                }
            } else {
                let tags = if let Some(key) = key {
                    HashMap::from([(key, value)])
                } else {
                    HashMap::new()
                };

                let mut swapping = EntryPartial {
                    id,
                    uid: record.get(1),
                    journals_id: record.get(2),
                    users_id: record.get(3),
                    title: record.get(4),
                    date: record.get(5),
                    created: record.get(6),
                    updated: record.get(7),
                    tags
                };

                std::mem::swap(&mut swapping, curr);

                found.push(swapping);
            }
        } else {
            let tags = if let Some(key) = key {
                HashMap::from([(key, value)])
            } else {
                HashMap::new()
            };

            current = Some(EntryPartial {
                id: record.get(0),
                uid: record.get(1),
                journals_id: record.get(2),
                users_id: record.get(3),
                title: record.get(4),
                date: record.get(5),
                created: record.get(6),
                updated: record.get(7),
                tags
            });
        }
    }

    if let Some(curr) = current {
        found.push(curr);
    }

    Ok(body::Json(found).into_response())
}

#[derive(Debug, Deserialize)]
pub struct MaybeEntryDate {
    date: Option<NaiveDate>
}

#[derive(Debug, Deserialize)]
pub struct EntryDate {
    date: NaiveDate
}

pub async fn retrieve_entry(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
    Path(MaybeEntryDate { date }): Path<MaybeEntryDate>,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let Some(date) = date else {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator_pg!(&conn, &headers, Some(uri));

    let result = Journal::retrieve_default_pg(&conn, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    auth::perm_check_pg!(&conn, initiator, journal, Scope::Entries, Ability::Read);

    let result = EntryFull::retrieve_date_pg(
        &conn,
        journal.id,
        initiator.user.id,
        &date
    )
        .await
        .context("failed to retrieve journal entry for date")?;

    let Some(entry) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    tracing::debug!("entry: {entry:#?}");

    Ok(body::Json(entry).into_response())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientData {
    key: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Attached<T, E> {
    #[serde(flatten)]
    inner: T,
    attached: E,
}

impl<T, E> From<(T, E)> for Attached<T, E> {
    fn from((inner, attached): (T, E)) -> Self {
        Self { inner, attached }
    }
}

pub type ResultFileEntry = Attached<FileEntry, ClientData>;
pub type ResultEntryFull = EntryFull<ResultFileEntry>;

#[derive(Debug, Deserialize)]
pub struct NewEntryBody {
    date: NaiveDate,
    title: Option<String>,
    contents: Option<String>,
    tags: Vec<TagEntryBody>,
    files: Vec<NewFileEntryBody>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatedEntryBody {
    date: NaiveDate,
    title: Option<String>,
    contents: Option<String>,
    tags: Vec<TagEntryBody>,
    files: Vec<UpdatedFileEntryBody>,
}

#[derive(Debug, Deserialize)]
pub struct TagEntryBody {
    key: String,
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExistingFileEntryBody {
    id: FileEntryId,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewFileEntryBody {
    key: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub enum UpdatedFileEntryBody {
    New(NewFileEntryBody),
    Existing(ExistingFileEntryBody),
}

fn non_empty_str(given: String) -> Option<String> {
    let trimmed = given.trim();

    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn opt_non_empty_str(given: Option<String>) -> Option<String> {
    if let Some(value) = given {
        non_empty_str(value)
    } else {
        None
    }
}

pub async fn create_entry(
    state: state::SharedState,
    headers: HeaderMap,
    body::Json(json): body::Json<NewEntryBody>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator_pg!(&transaction, &headers, None::<Uri>);

    let result = Journal::retrieve_default_pg(&transaction, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    auth::perm_check_pg!(&transaction, initiator, journal, Scope::Entries, Ability::Create);

    let uid = EntryUid::gen();
    let journals_id = journal.id;
    let users_id = initiator.user.id;
    let entry_date = json.date;
    let title = opt_non_empty_str(json.title);
    let contents = opt_non_empty_str(json.contents);
    let created = Utc::now();

    let id: EntryId = {
        let result = transaction.query_one(
            "\
            insert into entries (uid, journals_id, users_id, entry_date, title, contents, created) \
            values ($1, $2, $3, $4, $5, $6, $7) \
            returning id",
            &[&uid, &journals_id, &users_id, &entry_date, &title, &contents, &created]
        )
            .await
            .context("failed to insert entry into database")?;

        result.get(0)
    };

    let tags = if !json.tags.is_empty() {
        let mut first = true;
        let mut rtn: Vec<EntryTag> = Vec::new();

        for tag in json.tags {
            let Some(key) = non_empty_str(tag.key) else {
                continue;
            };
            let value = opt_non_empty_str(tag.value);

            rtn.push(EntryTag {
                key,
                value,
                created,
                updated: None
            });
        }

        let mut params: db::ParamsVec<'_> = vec![&id, &created];
        let mut query = String::from(
            "insert into entry_tags (entries_id, key, value, created) values "
        );

        for tag in &rtn {
            if first {
                first = false;
            } else {
                query.push_str(", ");
            }

            write!(
                &mut query,
                "($1, ${}, ${}, $2)",
                db::push_param(&mut params, &tag.key),
                db::push_param(&mut params, &tag.value)
            ).unwrap();
        }

        transaction.execute(query.as_str(), params.as_slice())
            .await
            .context("failed to commit tags")?;

        rtn
    } else {
        Vec::new()
    };

    let mut created_files = Vec::new();

    let files = if !json.files.is_empty() {
        let mut first = true;
        let mut rtn: Vec<ResultFileEntry> = Vec::new();

        for file in json.files {
            let uid = FileEntryUid::gen();
            let name = opt_non_empty_str(file.name);
            let mime_type = String::from("");
            let mime_subtype = String::from("");
            let created = created;

            let file_entry = FileEntry {
                id: FileEntryId::new(1).unwrap(),
                uid,
                entries_id: id,
                name,
                mime_type,
                mime_subtype,
                mime_param: None,
                size: 0,
                created,
                updated: None
            };
            let client_data = ClientData {
                key: file.key
            };

            rtn.push(ResultFileEntry::from((file_entry, client_data)));
        }

        let mut params: db::ParamsVec<'_> = vec![&id, &created];
        let mut query = String::from(
            "insert into file_entries ( \
                uid, \
                entries_id, \
                name, \
                mime_type, \
                mime_subtype, \
                created \
            ) values "
        );

        for entry in &rtn {
            if first {
                first = false;
            } else {
                query.push_str(", ");
            }

            write!(
                &mut query,
                "(${}, $1, ${}, ${}, ${}, $2)",
                db::push_param(&mut params, &entry.inner.uid),
                db::push_param(&mut params, &entry.inner.name),
                db::push_param(&mut params, &entry.inner.mime_type),
                db::push_param(&mut params, &entry.inner.mime_subtype),
            ).unwrap();
        }

        query.push_str(" returning id");

        tracing::debug!("file insert query: \"{query}\"");

        let mut results = transaction.query_raw(query.as_str(), params)
            .await
            .context("failed to insert files")?;

        futures::pin_mut!(results);

        for file_entry in &mut rtn {
            let Some(ins_result) = results.next().await else {
                return Err(error::Error::context("less than expected number of ids returned from database"));
            };

            let record = ins_result.context("failed to retrieve file entry id from insert")?;

            let file_entry_id = record.get(0);
            file_entry.inner.id = file_entry_id;

            let file_path = state.storage()
                .journal_file_entry(journal.id, file_entry_id);
            let file_result = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&file_path)
                .await;

            match file_result {
                Ok(_) => created_files.push(file_path),
                Err(err) => {
                    let failed = files::drop_files(created_files).await;

                    for (path, err) in failed {
                        tracing::error!("failed to remove journal file: \"{}\" {err}", path.display());
                    }

                    return Err(error::Error::context_source(
                        "failed to create file for journal entry",
                        err
                    ));
                }
            }
        }

        rtn
    } else {
        Vec::new()
    };

    let commit_result = transaction.commit()
        .await;

    if let Err(err) = commit_result {
        let failed = files::drop_files(created_files).await;

        for (path, err) in failed {
            tracing::error!("failed to remove journal file: \"{}\" {err}", path.display());
        }

        return Err(error::Error::context_source(
            "failed to commit changes to journal entry",
            err
        ));
    }

    let entry = ResultEntryFull {
        id,
        uid,
        journals_id,
        users_id,
        date: entry_date,
        title,
        contents,
        created,
        updated: None,
        tags,
        files,
    };

    Ok((
        StatusCode::CREATED,
        body::Json(entry),
    ).into_response())
}

pub async fn update_entry(
    state: state::SharedState,
    headers: HeaderMap,
    Path(EntryDate { date }): Path<EntryDate>,
    body::Json(json): body::Json<UpdatedEntryBody>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator_pg!(&transaction, &headers, None::<Uri>);
    let result = Journal::retrieve_default_pg(&transaction, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    auth::perm_check_pg!(&transaction, initiator, journal, Scope::Entries, Ability::Update);

    let result = Entry::retrieve_date_pg(
        &transaction,
        journal.id,
        initiator.user.id,
        &date
    )
        .await
        .context("failed to retrieve journal entry by date")?;

    let Some(entry) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    tracing::debug!("entry: {entry:#?}");

    let entry_date = json.date;
    let title = opt_non_empty_str(json.title);
    let contents = opt_non_empty_str(json.contents);
    let updated = Utc::now();

    transaction.execute(
        "\
        update entries \
        set entry_date = $2, \
            title = $3, \
            contents = $4, \
            updated = $5 \
        where id = $1",
        &[&entry.id, &entry_date, &title, &contents, &updated]
    )
        .await
        .context("failed to update journal entry")?;

    let tags = {
        let mut tags: Vec<EntryTag> = Vec::new();
        let mut unchanged: Vec<EntryTag> = Vec::new();
        let mut current_tags: HashMap<String, EntryTag> = HashMap::new();

        {
            let mut tag_stream = EntryTag::retrieve_entry_stream_pg(&transaction, entry.id)
                .await
                .context("failed to retrieve entry tags")?;

            futures::pin_mut!(tag_stream);

            while let Some(tag_result) = tag_stream.next().await {
                let tag = tag_result.context("failed to retrieve journal tag")?;

                current_tags.insert(tag.key.clone(), tag);
            }
        }

        for tag in json.tags {
            let Some(key) = non_empty_str(tag.key) else {
                continue;
            };
            let value = opt_non_empty_str(tag.value);

            if let Some(mut found) = current_tags.remove(&key) {
                if found.value != value {
                    found.value = value.clone();
                    found.updated = Some(updated);

                    tags.push(found);
                } else {
                    unchanged.push(found);
                }
            } else {
                tags.push(EntryTag {
                    key: key.clone(),
                    value: value.clone(),
                    created: updated,
                    updated: None,
                });
            }
        }

        if !tags.is_empty() {
            let mut first = true;
            let mut params: db::ParamsVec<'_> = vec![&entry.id, &updated];
            let mut query = String::from(
                "insert into entry_tags (entries_id, key, value, created) values "
            );

            for tag in &tags {

                if first {
                    first = false;
                } else {
                    query.push_str(", ");
                }

                write!(
                    &mut query,
                    "($1, ${}, ${}, $2)",
                    db::push_param(&mut params, &tag.key),
                    db::push_param(&mut params, &tag.value),
                ).unwrap();
            }

            query.push_str(" on conflict (entries_id, key) do update set \
                value = EXCLUDED.value, \
                updated = EXCLUDED.created");

            transaction.execute(query.as_str(), params.as_slice())
                .await
                .context("failed to upsert tags for journal")?;
        }

        if !current_tags.is_empty() {
            let keys: Vec<String> = current_tags.into_keys()
                .collect();

            transaction.execute(
                "\
                delete from entry_tags \
                where entries_id = $1 and \
                      key = any($2)",
                &[&entry.id, &keys]
            )
                .await
                .context("failed to delete tags for journal")?;

            /*
            let mut first = true;
            let mut params: db::ParamsVec<'_> = vec![&entry.id];
            let mut query = String::from(
                "delete from entry_tags where entries_id = $1 and key in ("
            );

            for (key, _) in &current_tags {
                if first {
                    first = false;

                    write!(
                        &mut query,
                        "{}",
                        db::push_param(&mut params, key)
                    ).unwrap();
                } else {
                    write!(
                        &mut query,
                        ",{}",
                        db::push_param(&mut params, key)
                    ).unwrap();
                }
            }

            query.push_str(")");

            transaction.execute(query.as_str(), params.as_slice())
                .await
                .context("failed to delete tags for journal")?;
                */
        }

        tags.extend(unchanged);
        tags
    };

    transaction.commit()
        .await
        .context("failed commit changes to journal entry")?;

    let entry = ResultEntryFull {
        id: entry.id,
        uid: entry.uid,
        journals_id: entry.journals_id,
        users_id: entry.users_id,
        date: entry_date,
        title,
        contents,
        created: entry.created,
        updated: Some(updated),
        tags,
        files: Vec::new(),
    };

    Ok(body::Json(entry).into_response())
}

pub async fn delete_entry(
    state: state::SharedState,
    headers: HeaderMap,
    Path(EntryDate { date }): Path<EntryDate>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator_pg!(&transaction, &headers, None::<Uri>);
    let result = Journal::retrieve_default_pg(&transaction, initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    auth::perm_check_pg!(&transaction, initiator, journal, Scope::Entries, Ability::Delete);

    let result = EntryFull::retrieve_date_pg(
        &transaction,
        journal.id,
        initiator.user.id,
        &date
    )
        .await
        .context("failed to retrieve journal entry by date")?;

    let Some(entry) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let tags = transaction.execute(
        "delete from entry_tags where entries_id = $1",
        &[&entry.id]
    )
        .await
        .context("failed to delete tags for journal entry")?;

    if tags != entry.tags.len() as u64 {
        tracing::warn!("dangling tags for journal entry");
    }

    let _files = transaction.execute(
        "delete from file_entries where entries_id = $1",
        &[&entry.id]
    )
        .await
        .context("failed to delete files for journal entry")?;

    let marked_files = if !entry.files.is_empty() {
        let files_dir = state.storage().journal_files(journal.id);
        let result = files::mark_remove(&files_dir,entry.files).await;

        match result {
            Ok(successful) => successful,
            Err((processed, err)) => {
                let failed = files::unmark_remove(processed).await;

                for (path, err) in failed {
                    tracing::error!("failed to unmark journal file: \"{}\" {err}", path.display());
                }

                return Err(error::Error::context_source(
                    "failed to mark files for removal",
                    err
                ));
            }
        }
    } else {
        Vec::new()
    };

    let entry_result = transaction.execute(
        "delete from entries where id = $1",
        &[&entry.id]
    ).await;

    match entry_result {
        Ok(execed) => {
            if execed != 1 {
                tracing::warn!("did not find journal entry?");
            }
        }
        Err(err) => {
            if !marked_files.is_empty() {
                let failed = files::unmark_remove(marked_files).await;

                for (path, err) in failed {
                    tracing::error!("failed to unmark journal file: \"{}\" {err}", path.display());
                }
            }

            return Err(error::Error::context_source(
                "failed to delete entry for journal",
                err
            ));
        }
    }

    if let Err(err) = transaction.commit().await {
        if !marked_files.is_empty() {
            let failed = files::unmark_remove(marked_files).await;

            for (path, err) in failed {
                tracing::error!("failed to unmark journal file: \"{}\" {err}", path.display());
            }
        }

        Err(error::Error::context_source(
            "failed to commit changes to journal",
            err
        ))
    } else {
        if !marked_files.is_empty() {
            let failed = files::drop_marked(marked_files).await;

            for (path, err) in failed {
                tracing::error!("failed to removed marked journal file: \"{}\" {err}", path.display());
            }
        }

        Ok(StatusCode::OK.into_response())
    }
}
