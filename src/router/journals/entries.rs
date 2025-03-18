use std::collections::{HashSet, HashMap};
use std::fmt::Write;

use axum::extract::Path;
use axum::http::{StatusCode, Uri, HeaderMap};
use axum::response::{IntoResponse, Response};
use chrono::{NaiveDate, Utc, DateTime};
use futures::{Stream, StreamExt};
use serde::{Serialize, Deserialize};

use crate::state;
use crate::db;
use crate::db::ids::{
    EntryId,
    EntryUid,
    FileEntryId,
    FileEntryUid,
    JournalId,
    CustomFieldId
};
use crate::error::{self, Context};
use crate::fs::RemovedFiles;
use crate::journal::{
    custom_field,
    Journal,
    Entry,
    JournalDir,
    CustomField,
    EntryCreateError,
    FileStatus,
    FileEntry,
    ReceivedFile,
};
use crate::router::body;
use crate::router::macros;
use crate::sec::authz::{Scope, Ability};

mod auth;
mod search;

pub mod files;

pub use search::retrieve_entries;

#[derive(Debug, Deserialize)]
pub struct JournalPath {
    journals_id: JournalId,
}

#[derive(Debug, Deserialize)]
pub struct MaybeEntryPath {
    journals_id: JournalId,
    entries_id: Option<EntryId>,
}

#[derive(Debug, Deserialize)]
pub struct EntryPath {
    journals_id: JournalId,
    entries_id: EntryId,
}

#[derive(Debug, Serialize)]
pub struct EntryForm<FileT = EntryFileForm> {
    id: Option<EntryId>,
    uid: Option<EntryUid>,
    date: NaiveDate,
    title: Option<String>,
    contents: Option<String>,
    tags: Vec<EntryTagForm>,
    files: Vec<FileT>,
    custom_fields: Vec<EntryCustomFieldForm>,
}

impl EntryForm {
    pub async fn blank(
        conn: &impl db::GenericClient,
        journals_id: &JournalId,
    ) -> Result<Self, error::Error> {
        let now = Utc::now();
        let custom_fields = EntryCustomFieldForm::retrieve_empty(conn, journals_id).await?;

        Ok(EntryForm {
            id: None,
            uid: None,
            date: now.date_naive(),
            title: None,
            contents: None,
            tags: Vec::new(),
            files: Vec::new(),
            custom_fields,
        })
    }

    pub async fn retrieve_entry(
        conn: &impl db::GenericClient,
        journals_id: &JournalId,
        entries_id: &EntryId,
    ) -> Result<Option<Self>, error::Error> {
        let maybe = conn.query_opt(
            "\
            select entries.id, \
                   entries.uid, \
                   entries.entry_date, \
                   entries.title, \
                   entries.contents \
            from entries \
            where entries.journals_id = $1 and \
                  entries.id = $2",
            &[journals_id, entries_id]
        )
            .await
            .context("failed to retrieve entry")?;

        if let Some(found) = maybe {
            let (tags_res, files_res, custom_fields_res) = tokio::join!(
                EntryTagForm::retrieve_entry(conn, entries_id),
                EntryFileForm::retrieve_entry(conn, entries_id),
                EntryCustomFieldForm::retrieve_entry(conn, journals_id, entries_id),
            );

            let tags = tags_res?;
            let files = files_res?;
            let custom_fields = custom_fields_res?;

            Ok(Some(Self {
                id: found.get(0),
                uid: found.get(1),
                date: found.get(2),
                title: found.get(3),
                contents: found.get(4),
                tags,
                files,
                custom_fields,
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EntryTagForm {
    key: String,
    value: Option<String>,
}

impl EntryTagForm {
    pub async fn retrieve_entry_stream(
        conn: &impl db::GenericClient,
        entries_id: &EntryId
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [entries_id];

        let stream = conn.query_raw(
            "\
            select entry_tags.key, \
                   entry_tags.value \
            from entry_tags \
            where entry_tags.entries_id = $1",
            params
        ).await?;

        Ok(stream.map(|result| result.map(|record| Self {
            key: record.get(0),
            value: record.get(1),
        })))
    }

    pub async fn retrieve_entry(
        conn: &impl db::GenericClient,
        entries_id: &EntryId
    ) -> Result<Vec<Self>, error::Error> {
        let stream = Self::retrieve_entry_stream(conn, entries_id)
            .await
            .context("failed to retrieve entry tags")?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_record) = stream.next().await {
            rtn.push(try_record.context("failed to retrieve entry tag record")?);
        }

        Ok(rtn)
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum EntryFileForm {
    Requested {
        _id: FileEntryId,
        uid: FileEntryUid,
        name: Option<String>,
    },
    Received {
        _id: FileEntryId,
        uid: FileEntryUid,
        name: Option<String>,
        mime_type: String,
        mime_subtype: String,
        mime_param: Option<String>,
        size: i64,
    },
    Remote {
        _id: FileEntryId,
        uid: FileEntryUid,
        name: Option<String>,
        mime_type: String,
        mime_subtype: String,
        mime_param: Option<String>,
        size: i64,
    }
}

impl EntryFileForm {
    pub async fn retrieve_entry_stream(
        conn: &impl db::GenericClient,
        entries_id: &EntryId,
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        Ok(FileEntry::retrieve_entry_stream(conn, entries_id)
            .await?
            .map(|result| result.map(Into::into)))
    }

    pub async fn retrieve_entry(
        conn: &impl db::GenericClient,
        entries_id: &EntryId,
    ) -> Result<Vec<Self>, error::Error> {
        let stream = Self::retrieve_entry_stream(conn, entries_id)
            .await
            .context("failed to retrieve entry files")?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_record) = stream.next().await {
            rtn.push(try_record.context("failed to retrieve entry file record")?);
        }

        Ok(rtn)
    }

    pub async fn retrieve_entry_map(
        conn: &impl db::GenericClient,
        entries_id: &EntryId,
    ) -> Result<HashMap<FileEntryId, Self>, error::Error> {
        let stream = Self::retrieve_entry_stream(conn, entries_id)
            .await
            .context("failed to retrieve entry files")?;

        futures::pin_mut!(stream);

        let mut rtn = HashMap::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record.context("failed to retrieve entry file record")?;

            rtn.insert(*record.id(), record);
        }

        Ok(rtn)
    }

    fn id(&self) -> &FileEntryId {
        match self {
            Self::Requested { _id, .. } |
            Self::Received { _id, .. } |
            Self::Remote { _id, .. } => _id
        }
    }

    fn is_received(&self) -> bool {
        match self {
            Self::Received { .. } => true,
            _ => false
        }
    }
}

impl From<FileEntry> for EntryFileForm {
    fn from(given: FileEntry) -> Self {
        match given {
            FileEntry::Requested(req) => Self::Requested {
                _id: req.id,
                uid: req.uid,
                name: req.name,
            },
            FileEntry::Received(rec) => Self::Received {
                _id: rec.id,
                uid: rec.uid,
                name: rec.name,
                mime_type: rec.mime_type,
                mime_subtype: rec.mime_subtype,
                mime_param: rec.mime_param,
                size: rec.size,
            },
            FileEntry::Remote(rmt) => Self::Remote {
                _id: rmt.id,
                uid: rmt.uid,
                name: rmt.name,
                mime_type: rmt.mime_type,
                mime_subtype: rmt.mime_subtype,
                mime_param: rmt.mime_param,
                size: rmt.size,
            }
        }
    }
}

impl From<ReceivedFile> for EntryFileForm {
    fn from(given: ReceivedFile) -> Self {
        Self::Received {
            _id: given.id,
            uid: given.uid,
            name: given.name,
            mime_type: given.mime_type,
            mime_subtype: given.mime_subtype,
            mime_param: given.mime_param,
            size: given.size,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CFTypeForm<T, V> {
    _id: CustomFieldId,
    enabled: bool,
    order: i32,
    name: String,
    description: Option<String>,
    config: T,
    value: V,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum EntryCustomFieldForm {
    Integer(CFTypeForm<custom_field::IntegerType, custom_field::IntegerValue>),
    IntegerRange(CFTypeForm<custom_field::IntegerRangeType, custom_field::IntegerRangeValue>),
    Float(CFTypeForm<custom_field::FloatType, custom_field::FloatValue>),
    FloatRange(CFTypeForm<custom_field::FloatRangeType, custom_field::FloatRangeValue>),
    Time(CFTypeForm<custom_field::TimeType, custom_field::TimeValue>),
    TimeRange(CFTypeForm<custom_field::TimeRangeType, custom_field::TimeRangeValue>),
}

impl EntryCustomFieldForm {
    pub async fn retrieve_empty(
        conn: &impl db::GenericClient,
        journals_id: &JournalId,
    ) -> Result<Vec<Self>, error::Error> {
        let params: db::ParamsArray<'_, 1> = [journals_id];
        let stream = conn.query_raw(
            "\
            select custom_fields.id, \
                   custom_fields.name, \
                   custom_fields.config, \
                   custom_fields.description, \
                   custom_fields.\"order\" \
            from custom_fields \
            where custom_fields.journals_id = $1 \
            order by custom_fields.\"order\" desc,
                     custom_fields.name",
            params
        )
            .await
            .context("failed to retrieve custom fields for entry")?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_row) = stream.next().await {
            let row = try_row.context("failed to retrieve custom field entry record")?;
            let _id = row.get(0);
            let enabled = false;
            let name = row.get(1);
            let ty = row.get(2);
            let description = row.get(3);
            let order = row.get(4);

            rtn.push(match ty {
                custom_field::Type::Integer(config) => {
                    let value = config.make_value();

                    Self::Integer(CFTypeForm {
                        _id,
                        enabled,
                        order,
                        name,
                        description,
                        config,
                        value
                    })
                }
                custom_field::Type::IntegerRange(config) => {
                    let value = config.make_value();

                    Self::IntegerRange(CFTypeForm {
                        _id,
                        enabled,
                        order,
                        name,
                        description,
                        config,
                        value,
                    })
                }
                custom_field::Type::Float(config) => {
                    let value = config.make_value();

                    Self::Float(CFTypeForm {
                        _id,
                        enabled,
                        order,
                        name,
                        description,
                        config,
                        value,
                    })
                }
                custom_field::Type::FloatRange(config) => {
                    let value = config.make_value();

                    Self::FloatRange(CFTypeForm {
                        _id,
                        enabled,
                        order,
                        name,
                        description,
                        config,
                        value,
                    })
                }
                custom_field::Type::Time(config) => {
                    let value = config.make_value();

                    Self::Time(CFTypeForm {
                        _id,
                        enabled,
                        order,
                        name,
                        description,
                        config,
                        value,
                    })
                }
                custom_field::Type::TimeRange(config) => {
                    let value = config.make_value();

                    Self::TimeRange(CFTypeForm {
                        _id,
                        enabled,
                        order,
                        name,
                        description,
                        config,
                        value,
                    })
                }
            });
        }

        Ok(rtn)
    }

    pub fn get_record(
        _id: CustomFieldId,
        order: i32,
        name: String,
        description: Option<String>,
        ty: custom_field::Type,
        v: Option<custom_field::Value>
    ) -> Self {
        match ty {
            custom_field::Type::Integer(config) => {
                let mapped = v.map(|exists| exists.try_into()
                    .expect("failed to convert custom field entry into integer value"));

                let (enabled, value) = if let Some(value) = mapped {
                    (true, value)
                } else {
                    (false, config.make_value())
                };

                Self::Integer(CFTypeForm {
                    _id,
                    enabled,
                    order,
                    name,
                    description,
                    config,
                    value,
                })
            }
            custom_field::Type::IntegerRange(config) => {
                let mapped = v.map(|exists| exists.try_into()
                    .expect("failed to convert custom field entry into integer range value"));

                let (enabled, value) = if let Some(value) = mapped {
                    (true, value)
                } else {
                    (false, config.make_value())
                };

                Self::IntegerRange(CFTypeForm {
                    _id,
                    enabled,
                    order,
                    name,
                    description,
                    config,
                    value,
                })
            }
            custom_field::Type::Float(config) => {
                let mapped = v.map(|exists| exists.try_into()
                    .expect("failed to convert custom field entry into float value"));

                let (enabled, value) = if let Some(value) = mapped {
                    (true, value)
                } else {
                    (false, config.make_value())
                };

                Self::Float(CFTypeForm {
                    _id,
                    enabled,
                    order,
                    name,
                    description,
                    config,
                    value,
                })
            }
            custom_field::Type::FloatRange(config) => {
                let mapped = v.map(|exists| exists.try_into()
                    .expect("failed to convert custom field entry into float range value"));

                let (enabled, value) = if let Some(value) = mapped {
                    (true, value)
                } else {
                    (false, config.make_value())
                };

                Self::FloatRange(CFTypeForm {
                    _id,
                    enabled,
                    order,
                    name,
                    description,
                    config,
                    value,
                })
            }
            custom_field::Type::Time(config) => {
                let mapped = v.map(|exists| exists.try_into()
                    .expect("failed to convert custom field entry into time value"));

                let (enabled, value) = if let Some(value) = mapped {
                    (true, value)
                } else {
                    (false, config.make_value())
                };

                Self::Time(CFTypeForm {
                    _id,
                    enabled,
                    order,
                    name,
                    description,
                    config,
                    value,
                })
            }
            custom_field::Type::TimeRange(config) => {
                let mapped = v.map(|exists| exists.try_into()
                    .expect("failed to convert custom field entry into time range value"));

                let (enabled, value) = if let Some(value) = mapped {
                    (true, value)
                } else {
                    (false, config.make_value())
                };

                Self::TimeRange(CFTypeForm {
                    _id,
                    enabled,
                    order,
                    name,
                    description,
                    config,
                    value,
                })
            }
        }
    }

    pub async fn retrieve_entry_stream(
        conn: &impl db::GenericClient,
        journals_id: &JournalId,
        entries_id: &EntryId,
    ) -> Result<impl Stream<Item = Result<Self, error::Error>>, db::PgError> {
        let params: db::ParamsArray<'_, 2> = [journals_id, entries_id];
        let stream = conn.query_raw(
            "\
            select custom_fields.id, \
                   custom_fields.name, \
                   custom_fields.config, \
                   custom_fields.description, \
                   custom_fields.\"order\", \
                   custom_field_entries.value \
            from custom_fields \
                left join custom_field_entries on \
                    custom_fields.id = custom_field_entries.custom_fields_id and \
                    custom_field_entries.entries_id = $2 \
            where custom_fields.journals_id = $1 \
            order by custom_fields.\"order\" desc, \
                     custom_fields.name",
            params
        ).await?;

        Ok(stream.map(|try_record| try_record.map(|row| {
            Self::get_record(row.get(0), row.get(4), row.get(1), row.get(3), row.get(2), row.get(5))
        }).map_err(|err| error::Error::context_source(
            "failed to retrieve custom field record",
            err
        ))))
    }

    pub async fn retrieve_entry(
        conn: &impl db::GenericClient,
        journals_id: &JournalId,
        entries_id: &EntryId,
    ) -> Result<Vec<Self>, error::Error> {
        let stream = Self::retrieve_entry_stream(conn, journals_id, entries_id)
            .await
            .context("failed to retrieve custom fields for entry")?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_record) = stream.next().await {
            rtn.push(try_record?);
        }

        Ok(rtn)
    }
}

pub async fn retrieve_entry(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
    Path(MaybeEntryPath { journals_id, entries_id }): Path<MaybeEntryPath>,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator!(&conn, &headers, Some(uri));

    let result = Journal::retrieve_id(&conn, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    auth::perm_check!(&conn, initiator, journal, Scope::Entries, Ability::Read);

    if let Some(entries_id) = entries_id {
        let result = EntryForm::retrieve_entry(&conn, journal.id(), &entries_id)
            .await
            .context("failed to retrieve journal entry for date")?;

        let Some(entry) = result else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };

        tracing::debug!("entry: {entry:#?}");

        Ok(body::Json(entry).into_response())
    } else {
        let blank = EntryForm::blank(&conn, journal.id()).await?;

        Ok(body::Json(blank).into_response())
    }
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

pub type ResultFileEntry = Attached<EntryFileForm, Option<ClientData>>;
pub type ResultEntryFull = EntryForm<ResultFileEntry>;

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

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum CreateEntryResult {
    DateExists,
    JournalNotFound,
    NotLocalJournal,
    CustomFieldMismatch {
        mismatched: Vec<CustomFieldEntry>,
    },
    CustomFieldNotFound {
        ids: Vec<CustomFieldId>,
    },
    CustomFieldInvalid {
        invalid: Vec<CustomFieldEntry>,
    },
    CustomFieldDuplicates {
        ids: Vec<CustomFieldId>,
    },
    Created(ResultEntryFull)
}

pub async fn create_entry(
    state: state::SharedState,
    headers: HeaderMap,
    Path(JournalPath { journals_id }): Path<JournalPath>,
    body::Json(json): body::Json<NewEntryBody>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator!(&transaction, &headers, None::<Uri>);

    let journal = {
        let result = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
            .await
            .context("failed to retrieve default journal")?;

        let Some(journal) = result else {
            return Ok((
                StatusCode::NOT_FOUND,
                body::Json(CreateEntryResult::JournalNotFound)
            ).into_response());
        };

        let Ok(rtn) = journal.into_local() else {
            return Ok((
                StatusCode::BAD_REQUEST,
                body::Json(CreateEntryResult::NotLocalJournal)
            ).into_response());
        };

        rtn
    };

    auth::perm_check!(&transaction, initiator, journal, Scope::Entries, Ability::Create);

    let entry = {
        let mut options = Entry::create_options(journal.id, initiator.user.id, json.date);
        options.title = opt_non_empty_str(json.title);
        options.contents = opt_non_empty_str(json.contents);

        match Entry::create(&transaction, options).await {
            Ok(result) => result,
            Err(err) => match err {
                EntryCreateError::DateExists =>
                    return Ok((
                        StatusCode::BAD_REQUEST,
                        body::Json(CreateEntryResult::DateExists)
                    ).into_response()),
                _ => return Err(error::Error::context_source(
                    "failed to create entry",
                    err
                )),
            }
        }
    };

    let tags = if !json.tags.is_empty() {
        let mut rtn: Vec<EntryTagForm> = Vec::new();

        for tag in json.tags {
            let Some(key) = non_empty_str(tag.key) else {
                continue;
            };
            let value = opt_non_empty_str(tag.value);

            rtn.push(EntryTagForm {
                key,
                value,
            });
        }

        upsert_tags(&transaction, &entry.id, &entry.created, &rtn).await?;

        rtn
    } else {
        Vec::new()
    };

    let CustomFieldsUpsert {
        valid: custom_fields,
        not_found,
        mismatched,
        invalid,
        duplicates,
    } = upsert_custom_fields(
        &transaction,
        &journal.id,
        &entry.id,
        json.custom_fields
    ).await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(CreateEntryResult::CustomFieldNotFound {
                ids: not_found,
            })
        ).into_response());
    }

    if !mismatched.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(CreateEntryResult::CustomFieldMismatch {
                mismatched,
            })
        ).into_response());
    }

    if !invalid.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(CreateEntryResult::CustomFieldInvalid {
                invalid
            })
        ).into_response());
    }

    if !duplicates.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(CreateEntryResult::CustomFieldDuplicates {
                ids: duplicates,
            })
        ).into_response());
    }

    let dir = state.storage().journal_dir(journal.id);
    let mut removed_files = RemovedFiles::new();

    let files = upsert_files(
        &transaction,
        &dir,
        &entry.id,
        &entry.created,
        UpsertFilesKind::Creating(json.files),
        &mut removed_files
    ).await?;

    transaction.commit()
        .await
        .context("failed to commit changes to journal entry")?;

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

    Ok((
        StatusCode::CREATED,
        body::Json(CreateEntryResult::Created(entry)),
    ).into_response())
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum UpdateEntryResult {
    JournalNotFound,
    NotLocalJournal,
    CustomFieldMismatch {
        mismatched: Vec<CustomFieldEntry>,
    },
    CustomFieldNotFound {
        ids: Vec<CustomFieldId>,
    },
    CustomFieldInvalid {
        invalid: Vec<CustomFieldEntry>,
    },
    CustomFieldDuplicates {
        ids: Vec<CustomFieldId>,
    },
    Updated(ResultEntryFull)
}

pub async fn update_entry(
    state: state::SharedState,
    headers: HeaderMap,
    Path(EntryPath { journals_id, entries_id }): Path<EntryPath>,
    body::Json(json): body::Json<UpdatedEntryBody>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator!(&transaction, &headers, None::<Uri>);

    let journal = {
        let result = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
            .await
            .context("failed to retrieve default journal")?;

        let Some(journal) = result else {
            return Ok((
                StatusCode::NOT_FOUND,
                body::Json(UpdateEntryResult::JournalNotFound),
            ).into_response());
        };

        let Ok(rtn) = journal.into_local() else {
            return Ok((
                StatusCode::BAD_REQUEST,
                body::Json(UpdateEntryResult::NotLocalJournal),
            ).into_response());
        };

        rtn
    };

    auth::perm_check!(&transaction, initiator, journal, Scope::Entries, Ability::Update);

    let result = Entry::retrieve_id(
        &transaction,
        &journal.id,
        &initiator.user.id,
        &entries_id
    )
        .await
        .context("failed to retrieve journal entry by date")?;

    let Some(mut entry) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    entry.date = json.date;
    entry.title = opt_non_empty_str(json.title);
    entry.contents = opt_non_empty_str(json.contents);
    entry.updated = Some(Utc::now());

    entry.update(&transaction)
        .await
        .context("failed to update journal entry")?;

    let tags = {
        let mut tags: Vec<EntryTagForm> = Vec::new();
        let mut unchanged: Vec<EntryTagForm> = Vec::new();
        let mut current_tags: HashMap<String, EntryTagForm> = HashMap::new();

        let tag_stream = EntryTagForm::retrieve_entry_stream(&transaction, &entry.id)
            .await
            .context("failed to retrieve entry tags")?;

        futures::pin_mut!(tag_stream);

        while let Some(tag_result) = tag_stream.next().await {
            let tag = tag_result.context("failed to retrieve journal tag")?;

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
                tags.push(EntryTagForm {
                    key: key.clone(),
                    value: value.clone(),
                });
            }
        }

        if !tags.is_empty() {
            upsert_tags(&transaction, &entry.id, (entry.updated.as_ref()).unwrap(), &tags).await?;
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
        }

        tags.extend(unchanged);
        tags
    };

    let CustomFieldsUpsert {
        valid: custom_fields,
        not_found,
        mismatched,
        invalid,
        duplicates,
    } = upsert_custom_fields(
        &transaction,
        &journal.id,
        &entry.id,
        json.custom_fields
    ).await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdateEntryResult::CustomFieldNotFound {
                ids: not_found,
            })
        ).into_response());
    }

    if !mismatched.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdateEntryResult::CustomFieldMismatch {
                mismatched
            })
        ).into_response());
    }

    if !invalid.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdateEntryResult::CustomFieldInvalid {
                invalid
            })
        ).into_response());
    }

    if !duplicates.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdateEntryResult::CustomFieldDuplicates {
                ids: duplicates,
            })
        ).into_response());
    }

    let dir = state.storage().journal_dir(journal.id);
    let mut removed_files = RemovedFiles::new();

    let upsert_result = upsert_files(
        &transaction,
        &dir,
        &entry.id,
        entry.updated.as_ref().unwrap(),
        UpsertFilesKind::Updating(json.files),
        &mut removed_files
    ).await;

    let files = match upsert_result {
        Ok(files) => files,
        Err(err) => {
            removed_files.log_rollback().await;

            return Err(err);
        }
    };

    if let Err(err) = transaction.commit().await {
        removed_files.log_rollback().await;

        return Err(error::Error::context_source(
            "failed commit changes to journal entry",
            err
        ));
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

    Ok(body::Json(UpdateEntryResult::Updated(entry)).into_response())
}

pub async fn delete_entry(
    state: state::SharedState,
    headers: HeaderMap,
    Path(EntryPath { journals_id, entries_id }): Path<EntryPath>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator!(&transaction, &headers, None::<Uri>);

    let journal = {
        let result = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
            .await
            .context("failed to retrieve default journal")?;

        let Some(journal) = result else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };

        let Ok(rtn) = journal.into_local() else {
            return Ok(StatusCode::BAD_REQUEST.into_response());
        };

        rtn
    };

    auth::perm_check!(&transaction, initiator, journal, Scope::Entries, Ability::Delete);

    let result = Entry::retrieve_id(
        &transaction,
        &journal.id,
        &initiator.user.id,
        &entries_id
    )
        .await
        .context("failed to retrieve journal entry by date")?;

    let Some(entry) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let _tags = transaction.execute(
        "delete from entry_tags where entries_id = $1",
        &[&entry.id]
    )
        .await
        .context("failed to delete tags for journal entry")?;

    let _custom_fields = transaction.execute(
        "delete from custom_field_entries where entries_id = $1",
        &[&entry.id]
    )
        .await
        .context("failed to delete custom field entries for journal entry")?;

    let stream = transaction.query_raw(
        "delete from file_entries where entries_id = $1 returning id",
        &[&entry.id]
    )
        .await
        .context("failed to delete files for journal entry")?;

    futures::pin_mut!(stream);

    let mut marked_files = RemovedFiles::new();
    let journal_dir = state.storage().journal_dir(journal.id);

    while let Some(try_row) = stream.next().await {
        let row = try_row.context("failed to retrieve file entry row")?;
        let id: FileEntryId = row.get(0);

        let entry_path = journal_dir.file_path(&id);

        if let Err(err) = marked_files.add(entry_path).await {
            marked_files.log_rollback().await;

            return Err(error::Error::context_source(
                "failed to mark files for removal",
                err
            ));
        }
    }

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
                marked_files.log_rollback().await;
            }

            return Err(error::Error::context_source(
                "failed to delete entry for journal",
                err
            ));
        }
    }

    if let Err(err) = transaction.commit().await {
        if !marked_files.is_empty() {
            marked_files.log_rollback().await;
        }

        Err(error::Error::context_source(
            "failed to commit changes to journal",
            err
        ))
    } else {
        if !marked_files.is_empty() {
            marked_files.log_clean().await;
        }

        Ok(StatusCode::OK.into_response())
    }
}

enum UpsertFilesKind {
    Creating(Vec<NewFileEntryBody>),
    Updating(Vec<UpdatedFileEntryBody>),
}

async fn upsert_files(
    conn: &impl db::GenericClient,
    journal_dir: &JournalDir,
    entries_id: &EntryId,
    created: &DateTime<Utc>,
    given: UpsertFilesKind,
    removed_files: &mut RemovedFiles,
) -> Result<Vec<ResultFileEntry>, error::Error> {
    let mut files = Vec::new();
    let mut insert_indexs = Vec::new();
    let mut update_indexs = Vec::new();
    let existing;

    match given {
        UpsertFilesKind::Creating(given) => {
            for NewFileEntryBody { key, name } in given {
                insert_indexs.push(files.len());

                files.push(ResultFileEntry::from((
                    EntryFileForm::Requested {
                        _id: FileEntryId::zero(),
                        uid: FileEntryUid::gen(),
                        name: opt_non_empty_str(name)
                    },
                    Some(ClientData { key })
                )));
            }

            existing = None;
        }
        UpsertFilesKind::Updating(given) => {
            let mut current = EntryFileForm::retrieve_entry_map(conn, &entries_id)
                .await?;

            for file_entry in given {
                match file_entry {
                    UpdatedFileEntryBody::New(new) => {
                        insert_indexs.push(files.len());

                        files.push(ResultFileEntry::from((
                            EntryFileForm::Requested {
                                _id: FileEntryId::zero(),
                                uid: FileEntryUid::gen(),
                                name: opt_non_empty_str(new.name),
                            },
                            Some(ClientData {
                                key: new.key
                            })
                        )));
                    }
                    UpdatedFileEntryBody::Existing(exists) => {
                        let Some(mut found) = current.remove(&exists.id) else {
                            return Err(error::Error::context("a specified file does not exist in the database"));
                        };

                        let check = opt_non_empty_str(exists.name);

                        match &mut found {
                            EntryFileForm::Requested { name, .. } |
                            EntryFileForm::Received { name, .. } => if *name != check {
                                *name = check;

                                update_indexs.push(files.len());
                            }
                            // we are going to skip remotes as the server has
                            // no control over the data since the peer server
                            // controls the data
                            EntryFileForm::Remote { .. } => {}
                        }

                        files.push(ResultFileEntry::from((found, None)));
                    }
                }
            }

            existing = Some(current);
        }
    }

    if !insert_indexs.is_empty() {
        let status = FileStatus::Requested;
        let mut ins_params: db::ParamsVec<'_> = vec![entries_id, created];
        let mut ins_query = String::from(
            "\
            insert into file_entries ( \
                uid, \
                entries_id, \
                status, \
                name, \
                created, \
                mime_type, \
                mime_subtype, \
                hash \
            ) values "
        );

        for (index, ins_index) in insert_indexs.iter().enumerate() {
            if index != 0 {
                ins_query.push_str(", ");
            }

            match &files[*ins_index].inner {
                EntryFileForm::Requested { uid, name, .. } => {
                    write!(
                        &mut ins_query,
                        "(${}, $1, ${}, ${}, $2, '', '', '')",
                        db::push_param(&mut ins_params, uid),
                        db::push_param(&mut ins_params, &status),
                        db::push_param(&mut ins_params, name),
                    ).unwrap();
                }
                _ => unreachable!()
            }
        }

        ins_query.push_str(" returning id");

        let ins_results = conn.query_raw(&ins_query, ins_params)
            .await
            .context("failed to insert files")?
            .map(|result| match result {
                Ok(row) => Ok(row.get::<usize, FileEntryId>(0)),
                Err(err) => Err(error::Error::context_source(
                    "failed to retrieve file entry id from insert",
                    err
                ))
            });

        futures::pin_mut!(ins_results);

        for index in insert_indexs {
            let ins_result = ins_results.next()
                .await
                .context("less than expected number of ids returned")?;

            match &mut files[index].inner {
                EntryFileForm::Requested { _id, .. } => {
                    *_id = ins_result?;
                }
                _ => unreachable!(),
            }
        }
    }

    if !update_indexs.is_empty() {
        let mut upd_params: db::ParamsVec<'_> = vec![entries_id, created];
        let mut upd_query = String::from(
            "\
            update file_entries \
            set name = tmp_update.name, \
                updated = $2 \
            from (values "
        );

        for (index, upd_index) in update_indexs.iter().enumerate() {
            if index != 0 {
                upd_query.push_str(", ");
            }

            match &files[*upd_index].inner {
                EntryFileForm::Requested { _id, name, .. } |
                EntryFileForm::Received { _id, name, .. } => {
                    write!(
                        &mut upd_query,
                        "(${}, ${})",
                        db::push_param(&mut upd_params, _id),
                        db::push_param(&mut upd_params, name),
                    ).unwrap();
                }
                _ => unreachable!(),
            }
        }

        upd_query.push_str("\
            ) as tmp_update (id, name, updated) \
            where file_entries.id = tmp_update.id"
        );

        tracing::debug!("file update query: \"{upd_query}\"");

        conn.execute(&upd_query, upd_params.as_slice())
            .await
            .context("failed to update file entries")?;
    }

    if let Some(current) = existing.filter(|curr| !curr.is_empty()) {
        let mut to_delete = Vec::new();

        for (id, record) in &current {
            to_delete.push(id);

            if record.is_received() {
                removed_files.add(journal_dir.file_path(&id))
                    .await
                    .context("failed to remove file")?;
            }
        }

        conn.execute(
            "delete from file_entries where id = any($1)",
            &[&to_delete]
        )
            .await
            .context("failed to remove file entries")?;
    }

    Ok(files)
}

async fn upsert_tags(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
    created: &DateTime<Utc>,
    tags: &Vec<EntryTagForm>
) -> Result<(), error::Error> {
    let mut first = true;
    let mut params: db::ParamsVec<'_> = vec![entries_id, created];
    let mut query = String::from(
        "insert into entry_tags (entries_id, key, value, created) values "
    );

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
        ).unwrap();
    }

    query.push_str(" on conflict (entries_id, key) do update set \
        value = EXCLUDED.value, \
        updated = EXCLUDED.updated");

    conn.execute(query.as_str(), params.as_slice())
        .await
        .context("failed to upsert tags for journal")?;

    Ok(())
}

struct CustomFieldsUpsert {
    valid: Vec<EntryCustomFieldForm>,
    not_found: Vec<CustomFieldId>,
    mismatched: Vec<CustomFieldEntry>,
    invalid: Vec<CustomFieldEntry>,
    duplicates: Vec<CustomFieldId>,
}

async fn upsert_custom_fields(
    conn: &impl db::GenericClient,
    journals_id: &JournalId,
    entries_id: &EntryId,
    fields: Vec<CustomFieldEntry>,
) -> Result<CustomFieldsUpsert, error::Error> {
    let known = CustomField::retrieve_journal_map(conn, journals_id)
        .await
        .context("failed to retrieve journal custom fields")?;

    let mut existing = custom_field::retrieve_known_entry_ids(conn, entries_id)
        .await
        .context("failed to retrieve custom field entries ids")?;

    let created = Utc::now();
    let mut to_insert = Vec::new();
    let mut registered = HashSet::new();
    let mut not_found = Vec::new();
    let mut invalid = Vec::new();
    let mut duplicates = Vec::new();
    let mut mismatched = Vec::new();

    for mut field in fields {
        let Some(cf) = known.get(&field.custom_fields_id) else {
            not_found.push(field.custom_fields_id);

            continue;
        };

        if !registered.insert(field.custom_fields_id) {
            duplicates.push(field.custom_fields_id);

            continue;
        }

        existing.remove(&cf.id);

        match &cf.config {
            custom_field::Type::Integer(ty) => match field.value {
                custom_field::Value::Integer(check) => match ty.validate(check) {
                    Ok(valid) => {
                        field.value = valid.into();

                        to_insert.push(field);
                    }
                    Err(invalid_v) => {
                        field.value = invalid_v.into();

                        invalid.push(field);
                    }
                }
                _ => {
                    mismatched.push(field);
                }
            }
            custom_field::Type::IntegerRange(ty) => match field.value {
                custom_field::Value::IntegerRange(check) => match ty.validate(check) {
                    Ok(valid) => {
                        field.value = valid.into();

                        to_insert.push(field);
                    }
                    Err(invalid_v) => {
                        field.value = invalid_v.into();

                        invalid.push(field);
                    }
                }
                _ => {
                    mismatched.push(field);
                }
            }
            custom_field::Type::Float(ty) => match field.value {
                custom_field::Value::Float(check) => match ty.validate(check) {
                    Ok(valid) => {
                        field.value = valid.into();

                        to_insert.push(field);
                    }
                    Err(invalid_v) => {
                        field.value = invalid_v.into();

                        invalid.push(field);
                    }
                }
                _ => {
                    mismatched.push(field);
                }
            }
            custom_field::Type::FloatRange(ty) => match field.value {
                custom_field::Value::FloatRange(check) => match ty.validate(check) {
                    Ok(valid) => {
                        field.value = valid.into();

                        to_insert.push(field);
                    }
                    Err(invalid_v) => {
                        field.value = invalid_v.into();

                        invalid.push(field);
                    }
                }
                _ => {
                    mismatched.push(field);
                }
            }
            custom_field::Type::Time(ty) => match field.value {
                custom_field::Value::Time(check) => match ty.validate(check) {
                    Ok(valid) => {
                        field.value = valid.into();

                        to_insert.push(field);
                    },
                    Err(invalid_v) => {
                        field.value = invalid_v.into();

                        invalid.push(field);
                    }
                }
                _ => {
                    mismatched.push(field);
                }
            }
            custom_field::Type::TimeRange(ty) => match field.value {
                custom_field::Value::TimeRange(check) => match ty.validate(check) {
                    Ok(valid) => {
                        field.value = valid.into();

                        to_insert.push(field);
                    },
                    Err(invalid_v) => {
                        field.value = invalid_v.into();

                        invalid.push(field);
                    }
                }
                _ => {
                    mismatched.push(field);
                }
            }
        }
    }

    if !not_found.is_empty() || !invalid.is_empty() || !duplicates.is_empty() || !mismatched.is_empty() {
        return Ok(CustomFieldsUpsert {
            valid: Vec::new(),
            not_found,
            mismatched,
            invalid,
            duplicates,
        });
    }

    if !existing.is_empty() {
        let ids: Vec<CustomFieldId> = existing.into_iter().collect();

        conn.execute(
            "\
            delete from custom_field_entries \
            where custom_fields_id = any($1) and \
                  entries_id = $2",
            &[&ids, entries_id]
        )
            .await
            .context("failed to delete custom field entries")?;
    }

    let valid = if !to_insert.is_empty() {
        let mut first = true;
        let mut ins_query = String::from(
            "insert into custom_field_entries (custom_fields_id, entries_id, value, created) values"
        );
        let mut params: db::ParamsVec<'_> = vec![journals_id, entries_id, &created];

        for field in &to_insert {
            if first {
                first = false;
            } else {
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
                      value"
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

        let stream = conn.query_raw(&query, params)
            .await
            .context("failed to upsert custom field entries")?;

        futures::pin_mut!(stream);

        let mut records = Vec::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record.context("failed to retrieve upserted record")?;

            records.push(EntryCustomFieldForm::get_record(
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
        EntryCustomFieldForm::retrieve_empty(conn, journals_id).await?
    };

    Ok(CustomFieldsUpsert {
        valid,
        not_found,
        mismatched,
        invalid,
        duplicates,
    })
}
