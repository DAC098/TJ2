use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, NaiveDate, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{CustomFieldId, EntryId, FileEntryId, FileEntryUid, JournalId};
use crate::fs::RemovedFiles;
use crate::journal::{
    self, assert_permission, custom_field, sharing, CustomField, Entry, EntryCreateError, Journal,
    JournalDir, RequestedFile, RequestedFileBuilder,
};
use crate::net::body;
use crate::net::Error;
use crate::sec::authn::Initiator;
use crate::sec::authz::{Ability, Scope};
use crate::state;

mod search;

pub mod files;
pub mod form;

pub use search::search_entries;

#[derive(Debug, Deserialize)]
pub struct JournalPath {
    journals_id: JournalId,
}

#[derive(Debug, Deserialize)]
pub struct EntryPath {
    journals_id: JournalId,
    entries_id: EntryId,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum RetrieveBlankError {
    JournalNotFound,
}

impl IntoResponse for RetrieveBlankError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn retrieve_blank(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(JournalPath { journals_id }): Path<JournalPath>,
) -> Result<body::Json<form::EntryForm>, Error<RetrieveBlankError>> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db().get().await?;

    let journal = Journal::retrieve(&conn, &journals_id)
        .await?
        .ok_or(Error::Inner(RetrieveBlankError::JournalNotFound))?;

    assert_permission(
        &conn,
        &initiator,
        &journal,
        Scope::Entries,
        Ability::Read,
        sharing::Ability::EntryRead,
    )
    .await?;

    Ok(body::Json(
        form::EntryForm::blank(&conn, &journal.id).await?,
    ))
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum RetrieveError {
    JournalNotFound,
    EntryNotFound,
}

impl IntoResponse for RetrieveError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::EntryNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn retrieve_entry(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(EntryPath {
        journals_id,
        entries_id,
    }): Path<EntryPath>,
) -> Result<body::Json<form::EntryForm>, Error<RetrieveError>> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db().get().await?;

    let journal = Journal::retrieve(&conn, &journals_id)
        .await?
        .ok_or(Error::Inner(RetrieveError::JournalNotFound))?;

    assert_permission(
        &conn,
        &initiator,
        &journal,
        Scope::Entries,
        Ability::Read,
        sharing::Ability::EntryRead,
    )
    .await?;

    let rtn = form::EntryForm::retrieve_entry(&conn, &journal.id, &entries_id)
        .await?
        .ok_or(Error::Inner(RetrieveError::EntryNotFound))?;

    Ok(body::Json(rtn))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientData {
    key: String,
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

pub type ResultFileEntry = Attached<form::EntryFileForm, Option<ClientData>>;
pub type ResultEntryFull = form::EntryForm<ResultFileEntry>;

#[derive(Debug, Deserialize)]
pub struct NewEntryBody {
    date: NaiveDate,
    title: Option<String>,
    contents: Option<String>,
    tags: Vec<TagEntryBody>,
    files: Vec<NewFileEntryBody>,
    custom_fields: Vec<CustomFieldEntry>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatedEntryBody {
    date: NaiveDate,
    title: Option<String>,
    contents: Option<String>,
    tags: Vec<TagEntryBody>,
    files: Vec<UpdatedFileEntryBody>,
    custom_fields: Vec<CustomFieldEntry>,
}

#[derive(Debug, Deserialize)]
pub struct TagEntryBody {
    key: String,
    value: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomFieldEntry {
    custom_fields_id: CustomFieldId,
    value: custom_field::Value,
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
#[serde(untagged)]
pub enum UpdatedFileEntryBody {
    Existing(ExistingFileEntryBody),
    New(NewFileEntryBody),
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

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum CreateEntryError {
    DateExists,
    JournalNotFound,
    CustomFieldMismatch { mismatched: Vec<CustomFieldEntry> },
    CustomFieldNotFound { ids: Vec<CustomFieldId> },
    CustomFieldInvalid { invalid: Vec<CustomFieldEntry> },
    CustomFieldDuplicates { ids: Vec<CustomFieldId> },
}

impl IntoResponse for CreateEntryError {
    fn into_response(self) -> Response {
        match self {
            Self::DateExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::CustomFieldMismatch { .. } => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
            Self::CustomFieldNotFound { .. } => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
            Self::CustomFieldInvalid { .. } => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
            Self::CustomFieldDuplicates { .. } => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
        }
    }
}

pub async fn create_entry(
    state: state::SharedState,
    initiator: Initiator,
    Path(JournalPath { journals_id }): Path<JournalPath>,
    body::Json(json): body::Json<NewEntryBody>,
) -> Result<(StatusCode, body::Json<ResultEntryFull>), Error<CreateEntryError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let journal = Journal::retrieve(&transaction, &journals_id)
        .await?
        .ok_or(Error::Inner(CreateEntryError::JournalNotFound))?;

    assert_permission(
        &transaction,
        &initiator,
        &journal,
        Scope::Entries,
        Ability::Create,
        sharing::Ability::EntryCreate,
    )
    .await?;

    let entry = {
        let mut options = Entry::create_options(journal.id, journal.users_id, json.date);
        options.title = opt_non_empty_str(json.title);
        options.contents = opt_non_empty_str(json.contents);

        match Entry::create(&transaction, options).await {
            Ok(result) => result,
            Err(err) => {
                return match err {
                    EntryCreateError::DateExists => Err(Error::Inner(CreateEntryError::DateExists)),
                    _ => Err(Error::general().with_source(err)),
                }
            }
        }
    };

    let tags = if !json.tags.is_empty() {
        let mut rtn: Vec<form::EntryTagForm> = Vec::new();

        for tag in json.tags {
            let Some(key) = non_empty_str(tag.key) else {
                continue;
            };
            let value = opt_non_empty_str(tag.value);

            rtn.push(form::EntryTagForm { key, value });
        }

        upsert_tags(&transaction, &entry.id, &entry.created, &rtn).await?;

        rtn
    } else {
        Vec::new()
    };

    let custom_fields =
        create_custom_fields(&transaction, &journal.id, &entry.id, json.custom_fields).await?;

    let files = create_files(&transaction, &entry.id, &entry.created, json.files).await?;

    transaction.commit().await?;

    let entry = ResultEntryFull {
        id: Some(entry.id),
        uid: Some(entry.uid),
        date: entry.date,
        title: entry.title,
        contents: entry.contents,
        tags,
        files,
        custom_fields,
    };

    Ok((StatusCode::CREATED, body::Json(entry)))
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum UpdateEntryError {
    DateExists,
    JournalNotFound,
    EntryNotFound,
    FileNotFound { ids: Vec<FileEntryId> },
    CustomFieldMismatch { mismatched: Vec<CustomFieldEntry> },
    CustomFieldNotFound { ids: Vec<CustomFieldId> },
    CustomFieldInvalid { invalid: Vec<CustomFieldEntry> },
    CustomFieldDuplicates { ids: Vec<CustomFieldId> },
}

impl IntoResponse for UpdateEntryError {
    fn into_response(self) -> Response {
        match self {
            Self::DateExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::EntryNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::FileNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::CustomFieldMismatch { .. } => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
            Self::CustomFieldNotFound { .. } => {
                (StatusCode::NOT_FOUND, body::Json(self)).into_response()
            }
            Self::CustomFieldInvalid { .. } => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
            Self::CustomFieldDuplicates { .. } => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
        }
    }
}

pub async fn update_entry(
    state: state::SharedState,
    initiator: Initiator,
    Path(EntryPath {
        journals_id,
        entries_id,
    }): Path<EntryPath>,
    body::Json(json): body::Json<UpdatedEntryBody>,
) -> Result<body::Json<ResultEntryFull>, Error<UpdateEntryError>> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction().await?;

    let journal = Journal::retrieve(&transaction, &journals_id)
        .await?
        .ok_or(Error::Inner(UpdateEntryError::JournalNotFound))?;

    assert_permission(
        &transaction,
        &initiator,
        &journal,
        Scope::Entries,
        Ability::Update,
        sharing::Ability::EntryUpdate,
    )
    .await?;

    let mut entry = Entry::retrieve(&transaction, (&journal.id, &entries_id))
        .await?
        .ok_or(Error::Inner(UpdateEntryError::EntryNotFound))?;

    entry.date = json.date;
    entry.title = opt_non_empty_str(json.title);
    entry.contents = opt_non_empty_str(json.contents);
    entry.updated = Some(Utc::now());

    if let Err(err) = entry.update(&transaction).await {
        return Err(match err {
            journal::EntryUpdateError::DateExists => Error::Inner(UpdateEntryError::DateExists),
            journal::EntryUpdateError::NotFound => Error::general().with_source(err),
            journal::EntryUpdateError::Db(err) => Error::from(err),
        });
    }

    let tags = {
        let mut tags: Vec<form::EntryTagForm> = Vec::new();
        let mut unchanged: Vec<form::EntryTagForm> = Vec::new();
        let mut current_tags: HashMap<String, form::EntryTagForm> = HashMap::new();

        let tag_stream = form::EntryTagForm::retrieve_entry_stream(&transaction, &entry.id).await?;

        futures::pin_mut!(tag_stream);

        while let Some(tag_result) = tag_stream.next().await {
            let tag = tag_result?;

            current_tags.insert(tag.key.clone(), tag);
        }

        for tag in json.tags {
            let Some(key) = non_empty_str(tag.key) else {
                continue;
            };
            let value = opt_non_empty_str(tag.value);

            if let Some(mut found) = current_tags.remove(&key) {
                if found.value != value {
                    found.value = value.clone();

                    tags.push(found);
                } else {
                    unchanged.push(found);
                }
            } else {
                tags.push(form::EntryTagForm {
                    key: key.clone(),
                    value: value.clone(),
                });
            }
        }

        if !tags.is_empty() {
            upsert_tags(
                &transaction,
                &entry.id,
                (entry.updated.as_ref()).unwrap(),
                &tags,
            )
            .await?;
        }

        if !current_tags.is_empty() {
            let keys: Vec<String> = current_tags.into_keys().collect();

            transaction
                .execute(
                    "\
                delete from entry_tags \
                where entries_id = $1 and \
                      key = any($2)",
                    &[&entry.id, &keys],
                )
                .await?;
        }

        tags.extend(unchanged);
        tags
    };

    let custom_fields =
        upsert_custom_fields(&transaction, &journal.id, &entry.id, json.custom_fields).await?;

    let dir = state.storage().journal_dir(journal.id);
    let mut removed_files = RemovedFiles::new();

    let upsert_result = upsert_files(
        &transaction,
        &dir,
        &entry.id,
        entry.updated.as_ref().unwrap(),
        json.files,
        &mut removed_files,
    )
    .await;

    let files = match upsert_result {
        Ok(files) => files,
        Err(err) => {
            removed_files.log_rollback().await;

            return Err(err);
        }
    };

    if let Err(err) = transaction.commit().await {
        removed_files.log_rollback().await;

        return Err(Error::from(err));
    }

    removed_files.log_clean().await;

    let entry = ResultEntryFull {
        id: Some(entry.id),
        uid: Some(entry.uid),
        date: entry.date,
        title: entry.title,
        contents: entry.contents,
        tags,
        files,
        custom_fields,
    };

    Ok(body::Json(entry))
}

#[derive(Debug, strum::Display, Serialize)]
pub enum DeleteEntryError {
    JournalNotFound,
    EntryNotFound,
}

impl IntoResponse for DeleteEntryError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::EntryNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn delete_entry(
    state: state::SharedState,
    initiator: Initiator,
    Path(EntryPath {
        journals_id,
        entries_id,
    }): Path<EntryPath>,
) -> Result<StatusCode, Error<DeleteEntryError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let journal = Journal::retrieve(&transaction, &journals_id)
        .await?
        .ok_or(Error::Inner(DeleteEntryError::JournalNotFound))?;

    assert_permission(
        &transaction,
        &initiator,
        &journal,
        Scope::Entries,
        Ability::Delete,
        sharing::Ability::EntryDelete,
    )
    .await?;

    let entry = Entry::retrieve(&transaction, (&journal.id, &entries_id))
        .await?
        .ok_or(Error::Inner(DeleteEntryError::EntryNotFound))?;

    let _tags = transaction
        .execute("delete from entry_tags where entries_id = $1", &[&entry.id])
        .await?;

    let _custom_fields = transaction
        .execute(
            "delete from custom_field_entries where entries_id = $1",
            &[&entry.id],
        )
        .await?;

    let _synced_entries = transaction
        .execute(
            "delete from synced_entries where entries_id = $1",
            &[&entry.id],
        )
        .await?;

    let stream = transaction
        .query_raw(
            "delete from file_entries where entries_id = $1 returning id",
            &[&entry.id],
        )
        .await?;

    futures::pin_mut!(stream);

    let mut marked_files = RemovedFiles::new();
    let journal_dir = state.storage().journal_dir(journal.id);

    while let Some(try_row) = stream.next().await {
        let row = try_row?;
        let id: FileEntryId = row.get(0);

        let entry_path = journal_dir.file_path(&id);

        if let Err(err) = marked_files.add(entry_path).await {
            marked_files.log_rollback().await;

            return Err(Error::from(err));
        }
    }

    if let Err(err) = transaction
        .execute("delete from entries where id = $1", &[&entry.id])
        .await
    {
        if !marked_files.is_empty() {
            marked_files.log_rollback().await;
        }

        return Err(Error::from(err));
    }

    if let Err(err) = transaction.commit().await {
        if !marked_files.is_empty() {
            marked_files.log_rollback().await;
        }

        Err(Error::from(err))
    } else {
        if !marked_files.is_empty() {
            marked_files.log_clean().await;
        }

        Ok(StatusCode::OK)
    }
}

async fn create_files(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
    created: &DateTime<Utc>,
    given: Vec<NewFileEntryBody>,
) -> Result<Vec<ResultFileEntry>, Error<CreateEntryError>> {
    let mut files = Vec::new();
    let mut keys = Vec::new();

    for NewFileEntryBody { key, name } in given {
        let mut builder = RequestedFile::builder(*entries_id);
        builder.with_created(*created);
        builder.with_uid(FileEntryUid::gen());

        if let Some(name) = opt_non_empty_str(name) {
            builder.with_name(name);
        }

        files.push(builder);
        keys.push(ClientData { key });
    }

    if let Some(stream) = RequestedFileBuilder::build_many(conn, files).await? {
        let zipped = stream.zip(futures::stream::iter(keys));

        futures::pin_mut!(zipped);

        let mut rtn = Vec::new();

        while let Some((result, client)) = zipped.next().await {
            rtn.push(ResultFileEntry::from((
                form::EntryFileForm::from(result?),
                Some(client),
            )));
        }

        Ok(rtn)
    } else {
        Ok(Vec::new())
    }
}

async fn upsert_files(
    conn: &impl db::GenericClient,
    journal_dir: &JournalDir,
    entries_id: &EntryId,
    created: &DateTime<Utc>,
    given: Vec<UpdatedFileEntryBody>,
    removed_files: &mut RemovedFiles,
) -> Result<Vec<ResultFileEntry>, Error<UpdateEntryError>> {
    let mut current = form::EntryFileForm::retrieve_entry_map(conn, &entries_id).await?;

    let mut inserting = Vec::new();
    let mut updating = Vec::new();
    let mut keys = Vec::new();
    let mut not_found = Vec::new();
    let mut rtn = Vec::new();

    for file_entry in given {
        match file_entry {
            UpdatedFileEntryBody::New(NewFileEntryBody { key, name }) => {
                let mut builder = RequestedFile::builder(*entries_id);
                builder.with_created(*created);
                builder.with_uid(FileEntryUid::gen());

                if let Some(name) = opt_non_empty_str(name) {
                    builder.with_name(name);
                }

                keys.push(ClientData { key });
                inserting.push(builder);
            }
            UpdatedFileEntryBody::Existing(exists) => {
                let Some(mut found) = current.remove(&exists.id) else {
                    not_found.push(exists.id);

                    continue;
                };

                let check = opt_non_empty_str(exists.name);

                match &mut found {
                    form::EntryFileForm::Requested { name, .. }
                    | form::EntryFileForm::Received { name, .. } => {
                        if *name != check {
                            *name = check;

                            updating.push(ResultFileEntry::from((found, None)));
                        } else {
                            rtn.push(ResultFileEntry::from((found, None)));
                        }
                    }
                }
            }
        }
    }

    if !not_found.is_empty() {
        return Err(Error::Inner(UpdateEntryError::FileNotFound {
            ids: not_found,
        }));
    }

    let deleted_ids = if !current.is_empty() {
        let mut received_ids = Vec::new();
        let mut to_delete = Vec::new();

        for (id, record) in &current {
            to_delete.push(id);

            if record.is_received() {
                received_ids.push(id);
            }
        }

        conn.execute("delete from file_entries where id = any($1)", &[&to_delete])
            .await?;

        received_ids
    } else {
        Vec::new()
    };

    if let Some(stream) = RequestedFileBuilder::build_many(conn, inserting).await? {
        let zipped = stream.zip(futures::stream::iter(keys));

        futures::pin_mut!(zipped);

        while let Some((result, client)) = zipped.next().await {
            rtn.push(ResultFileEntry::from((
                form::EntryFileForm::from(result?),
                Some(client),
            )));
        }
    }

    if !updating.is_empty() {
        let mut upd_params: db::ParamsVec<'_> = vec![entries_id, created];
        let mut upd_query = String::from(
            "\
            update file_entries \
            set name = tmp_update.name, \
                updated = $2 \
            from (values ",
        );

        for (index, record) in updating.iter().enumerate() {
            if index != 0 {
                upd_query.push_str(", ");
            }

            match &record.inner {
                form::EntryFileForm::Requested { _id, name, .. }
                | form::EntryFileForm::Received { _id, name, .. } => {
                    write!(
                        &mut upd_query,
                        "(${}, ${})",
                        db::push_param(&mut upd_params, _id),
                        db::push_param(&mut upd_params, name),
                    )
                    .unwrap();
                }
            }
        }

        upd_query.push_str(
            "\
            ) as tmp_update (id, name, updated) \
            where file_entries.id = tmp_update.id",
        );

        tracing::debug!("file update query: \"{upd_query}\"");

        conn.execute(&upd_query, upd_params.as_slice()).await?;

        rtn.extend(updating);
    }

    for id in deleted_ids {
        removed_files.add(journal_dir.file_path(&id)).await?;
    }

    Ok(rtn)
}

async fn upsert_tags<T>(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
    created: &DateTime<Utc>,
    tags: &Vec<form::EntryTagForm>,
) -> Result<(), Error<T>> {
    let mut first = true;
    let mut params: db::ParamsVec<'_> = vec![entries_id, created];
    let mut query =
        String::from("insert into entry_tags (entries_id, key, value, created) values ");

    for tag in tags {
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
        )
        .unwrap();
    }

    query.push_str(
        " on conflict (entries_id, key) do update set \
        value = EXCLUDED.value, \
        updated = EXCLUDED.updated",
    );

    conn.execute(query.as_str(), params.as_slice()).await?;

    Ok(())
}

async fn create_custom_fields(
    conn: &impl db::GenericClient,
    journals_id: &JournalId,
    entries_id: &EntryId,
    fields: Vec<CustomFieldEntry>,
) -> Result<Vec<form::EntryCustomFieldForm>, Error<CreateEntryError>> {
    let known = CustomField::retrieve_journal_map(conn, journals_id).await?;

    let created = Utc::now();

    let mut to_insert = Vec::new();
    let mut registered = HashSet::new();
    let mut not_found = Vec::new();
    let mut invalid = Vec::new();
    let mut duplicates = Vec::new();
    let mut mismatched = Vec::new();

    for field in fields {
        let Some(cf) = known.get(&field.custom_fields_id) else {
            not_found.push(field.custom_fields_id);

            continue;
        };

        if !registered.insert(field.custom_fields_id) {
            duplicates.push(field.custom_fields_id);

            continue;
        }

        if let Err(err) = cf.config.validate(&field.value) {
            match err {
                custom_field::ValidationError::Mismatched => mismatched.push(field),
                custom_field::ValidationError::Invalid => invalid.push(field),
            }
        } else {
            to_insert.push(field);
        }
    }

    if !not_found.is_empty() {
        return Err(Error::Inner(CreateEntryError::CustomFieldNotFound {
            ids: not_found,
        }));
    }

    if !mismatched.is_empty() {
        return Err(Error::Inner(CreateEntryError::CustomFieldMismatch {
            mismatched,
        }));
    }

    if !invalid.is_empty() {
        return Err(Error::Inner(CreateEntryError::CustomFieldInvalid {
            invalid,
        }));
    }

    if !duplicates.is_empty() {
        return Err(Error::Inner(CreateEntryError::CustomFieldDuplicates {
            ids: duplicates,
        }));
    }

    if !to_insert.is_empty() {
        let mut ins_query = String::from(
            "insert into custom_field_entries (custom_fields_id, entries_id, value, created) values"
        );
        let mut params: db::ParamsVec<'_> = vec![journals_id, entries_id, &created];

        for (index, field) in to_insert.iter().enumerate() {
            if index != 0 {
                ins_query.push(',');
            }

            let fragment = format!(
                " (${}, $2, ${}, $3)",
                db::push_param(&mut params, &field.custom_fields_id),
                db::push_param(&mut params, &field.value),
            );

            ins_query.push_str(&fragment);
        }

        ins_query.push_str(
            " returning custom_fields_id, \
                        value",
        );

        let query = format!(
            "\
            with tmp_insert as ({ins_query}) \
            select custom_fields.id, \
                   custom_fields.\"order\", \
                   custom_fields.name, \
                   custom_fields.description, \
                   custom_fields.config, \
                   tmp_insert.value \
            from custom_fields \
                left join tmp_insert on \
                    custom_fields.id = tmp_insert.custom_fields_id \
            where custom_fields.journals_id = $1 \
            order by custom_fields.\"order\" desc, \
                     custom_fields.name"
        );

        tracing::debug!("upsert query: {query}");

        let stream = conn.query_raw(&query, params).await?;

        futures::pin_mut!(stream);

        let mut records = Vec::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record?;

            records.push(form::EntryCustomFieldForm::get_record(
                record.get(0),
                record.get(1),
                record.get(2),
                record.get(3),
                record.get(4),
                record.get(5),
            ));
        }

        Ok(records)
    } else {
        Ok(form::EntryCustomFieldForm::retrieve_empty(conn, journals_id).await?)
    }
}

async fn upsert_custom_fields(
    conn: &impl db::GenericClient,
    journals_id: &JournalId,
    entries_id: &EntryId,
    fields: Vec<CustomFieldEntry>,
) -> Result<Vec<form::EntryCustomFieldForm>, Error<UpdateEntryError>> {
    let known = CustomField::retrieve_journal_map(conn, journals_id).await?;

    let mut existing = custom_field::retrieve_known_entry_ids(conn, entries_id).await?;

    let created = Utc::now();
    let mut to_insert = Vec::new();
    let mut registered = HashSet::new();
    let mut not_found = Vec::new();
    let mut invalid = Vec::new();
    let mut duplicates = Vec::new();
    let mut mismatched = Vec::new();

    for field in fields {
        let Some(cf) = known.get(&field.custom_fields_id) else {
            not_found.push(field.custom_fields_id);

            continue;
        };

        if !registered.insert(field.custom_fields_id) {
            duplicates.push(field.custom_fields_id);

            continue;
        }

        existing.remove(&cf.id);

        if let Err(err) = cf.config.validate(&field.value) {
            match err {
                custom_field::ValidationError::Mismatched => mismatched.push(field),
                custom_field::ValidationError::Invalid => invalid.push(field),
            }
        } else {
            to_insert.push(field);
        }
    }

    if !not_found.is_empty() {
        return Err(Error::Inner(UpdateEntryError::CustomFieldNotFound {
            ids: not_found,
        }));
    }

    if !mismatched.is_empty() {
        return Err(Error::Inner(UpdateEntryError::CustomFieldMismatch {
            mismatched,
        }));
    }

    if !invalid.is_empty() {
        return Err(Error::Inner(UpdateEntryError::CustomFieldInvalid {
            invalid,
        }));
    }

    if !duplicates.is_empty() {
        return Err(Error::Inner(UpdateEntryError::CustomFieldDuplicates {
            ids: duplicates,
        }));
    }

    if !existing.is_empty() {
        let ids: Vec<CustomFieldId> = existing.into_iter().collect();

        conn.execute(
            "\
            delete from custom_field_entries \
            where custom_fields_id = any($1) and \
                  entries_id = $2",
            &[&ids, entries_id],
        )
        .await?;
    }

    let valid = if !to_insert.is_empty() {
        let mut ins_query = String::from(
            "insert into custom_field_entries (custom_fields_id, entries_id, value, created) values"
        );
        let mut params: db::ParamsVec<'_> = vec![journals_id, entries_id, &created];

        for (index, field) in to_insert.iter().enumerate() {
            if index != 0 {
                ins_query.push(',');
            }

            let fragment = format!(
                " (${}, $2, ${}, $3)",
                db::push_param(&mut params, &field.custom_fields_id),
                db::push_param(&mut params, &field.value),
            );

            ins_query.push_str(&fragment);
        }

        ins_query.push_str(
            " on conflict (custom_fields_id, entries_id) do update \
                set value = excluded.value, \
                    updated = excluded.created \
            returning custom_fields_id, \
                      value",
        );

        let query = format!(
            "\
            with tmp_insert as ({ins_query}) \
            select custom_fields.id, \
                   custom_fields.\"order\", \
                   custom_fields.name, \
                   custom_fields.description, \
                   custom_fields.config, \
                   tmp_insert.value \
            from custom_fields \
                left join tmp_insert on \
                    custom_fields.id = tmp_insert.custom_fields_id \
            where custom_fields.journals_id = $1 \
            order by custom_fields.\"order\" desc, \
                     custom_fields.name"
        );

        tracing::debug!("upsert query: {query}");

        let stream = conn.query_raw(&query, params).await?;

        futures::pin_mut!(stream);

        let mut records = Vec::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record?;

            records.push(form::EntryCustomFieldForm::get_record(
                record.get(0),
                record.get(1),
                record.get(2),
                record.get(3),
                record.get(4),
                record.get(5),
            ));
        }

        records
    } else {
        form::EntryCustomFieldForm::retrieve_empty(conn, journals_id).await?
    };

    Ok(valid)
}
