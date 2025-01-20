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
    UserId,
    CustomFieldId
};
use crate::error::{self, Context};
use crate::fs::{CreatedFiles, RemovedFiles};
use crate::journal::{
    custom_field,
    Journal,
    Entry,
    JournalDir,
    CustomField,
    EntryCreateError,
};
use crate::router::body;
use crate::router::macros;
use crate::sec::authz::{Scope, Ability};

mod auth;

pub mod files;

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
    uri: Uri,
    headers: HeaderMap,
    Path(JournalPath { journals_id }): Path<JournalPath>,
) -> Result<Response, error::Error> {
    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator!(
        &conn,
        &headers,
        Some(uri.clone())
    );

    macros::res_if_html!(state.templates(), &headers);

    let result = Journal::retrieve_id(&conn, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    auth::perm_check!(&conn, initiator, journal, Scope::Entries, Ability::Read);

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
               entry_tags.value \
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
    Server {
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
        let params: db::ParamsArray<'_, 1> = [entries_id];

        let stream = conn.query_raw(
            "\
            select file_entries.id, \
                   file_entries.uid, \
                   file_entries.name, \
                   file_entries.mime_type, \
                   file_entries.mime_subtype, \
                   file_entries.mime_param, \
                   file_entries.size \
            from file_entries \
            where file_entries.entries_id = $1",
            params
        ).await?;

        Ok(stream.map(|result| result.map(|record| Self::Server {
            _id: record.get(0),
            uid: record.get(1),
            name: record.get(2),
            mime_type: record.get(3),
            mime_subtype: record.get(4),
            mime_param: record.get(5),
            size: record.get(6),
        })))
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
        let result = EntryForm::retrieve_entry(&conn, &journal.id, &entries_id)
            .await
            .context("failed to retrieve journal entry for date")?;

        let Some(entry) = result else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };

        tracing::debug!("entry: {entry:#?}");

        Ok(body::Json(entry).into_response())
    } else {
        let blank = EntryForm::blank(&conn, &journal.id).await?;

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

    let result = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
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

    let (files, created_files) = if !json.files.is_empty() {
        let mut rtn: Vec<ResultFileEntry> = Vec::new();

        for file in json.files {
            let uid = FileEntryUid::gen();
            let name = opt_non_empty_str(file.name);
            let mime_type = String::from("");
            let mime_subtype = String::from("");

            let file_entry = EntryFileForm::Server {
                _id: FileEntryId::zero(),
                uid,
                name,
                mime_type,
                mime_subtype,
                mime_param: None,
                size: 0,
            };
            let client_data = ClientData {
                key: file.key
            };

            rtn.push(ResultFileEntry::from((file_entry, Some(client_data))));
        }

        let dir = state.storage().journal_dir(&journal);
        let created_files = insert_files(
            &transaction,
            &entry.id,
            &entry.created,
            &dir,
            &mut rtn
        ).await?;

        (rtn, created_files)
    } else {
        (Vec::new(), CreatedFiles::new())
    };

    let commit_result = transaction.commit()
        .await;

    if let Err(err) = commit_result {
        created_files.log_rollback().await;

        return Err(error::Error::context_source(
            "failed to commit changes to journal entry",
            err
        ));
    }

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

    let result = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
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

    let mut created_files = CreatedFiles::new();
    let mut removed_files = RemovedFiles::new();

    let files = {
        let journal_dir = state.storage()
            .journal_dir(&journal);
        let mut files = Vec::new();
        let mut new_files = Vec::new();
        let mut updated_files = Vec::new();
        let mut current = HashMap::new();
        let file_stream = EntryFileForm::retrieve_entry_stream(&transaction, &entry.id)
            .await
            .context("failed to retrieve file entries")?;

        futures::pin_mut!(file_stream);

        while let Some(file_result) = file_stream.next().await {
            let file = file_result.context("failed to retrieve file entry")?;
            let id = match &file {
                EntryFileForm::Server { _id, .. } => {*_id }
            };

            current.insert(id, file);
        }

        for file_entry in json.files {
            match file_entry {
                UpdatedFileEntryBody::New(new) => {
                    let uid = FileEntryUid::gen();
                    let name = opt_non_empty_str(new.name);
                    let mime_type = String::new();
                    let mime_subtype = String::new();

                    let file_entry = EntryFileForm::Server {
                        _id: FileEntryId::zero(),
                        uid,
                        name,
                        mime_type,
                        mime_subtype,
                        mime_param: None,
                        size: 0,
                    };
                    let client_data = ClientData {
                        key: new.key
                    };

                    new_files.push(ResultFileEntry::from((file_entry, Some(client_data))));
                }
                UpdatedFileEntryBody::Existing(exists) => {
                    let Some(mut found) = current.remove(&exists.id) else {
                        return Err(error::Error::context("a specified file does not exist in the database"));
                    };

                    let check = opt_non_empty_str(exists.name);

                    match &mut found {
                        EntryFileForm::Server { name, .. } => {
                            if *name == check {
                                files.push(ResultFileEntry::from((found, None)));
                            } else {
                                *name = check;

                                updated_files.push(ResultFileEntry::from((found, None)));
                            }
                        }
                    }

                }
            }
        }

        if !new_files.is_empty() {
            let dir = state.storage().journal_dir(&journal);

            created_files = insert_files(
                &transaction,
                &entry.id,
                (entry.updated.as_ref()).unwrap(),
                &dir,
                &mut new_files
            ).await?;
            files.extend(new_files);
        }

        if !updated_files.is_empty() {
            let mut failed = false;

            {
                let mut futs = futures::stream::FuturesUnordered::new();

                for file in &updated_files {
                    match &file.inner {
                        EntryFileForm::Server { _id, name, .. } => {
                            let params: db::ParamsArray<'_, 2> = [_id, name];

                            futs.push(transaction.execute_raw(
                                "\
                                update file_entries \
                                set name = $2 \
                                where id = $1",
                                params
                            ));
                        }
                    }
                }

                while let Some(result) = futs.next().await {
                    match result {
                        Ok(count) => if count != 1 {
                            tracing::debug!("failed to update file entry?");
                        }
                        Err(err) => {
                            failed = true;

                            error::log_prefix_error(
                                "failed to udpate file entry", &err
                            );
                        }
                    }
                }
            }

            if failed {
                return Err(error::Error::context(
                    "failed to update file entries"
                ));
            } else {
                files.extend(updated_files);
            }
        }

        if !current.is_empty() {
            let mut to_delete = Vec::new();

            for (id, _record) in &current {
                to_delete.push(id);

                if let Err(err) = removed_files.add(journal_dir.file_path(&id)).await {
                    created_files.log_rollback().await;
                    removed_files.log_rollback().await;

                    return Err(error::Error::context_source(
                        "failed to remove file",
                        err
                    ));
                }
            }

            let result = transaction.execute(
                "delete from file_entries where id = any($1)",
                &[&to_delete]
            ).await;

            match result {
                Ok(_affected) => {},
                Err(err) => {
                    created_files.log_rollback().await;
                    removed_files.log_rollback().await;

                    return Err(error::Error::context_source(
                        "failed to remove file entries",
                        err
                    ));
                }
            }
        }

        files
    };

    let commit_result = transaction.commit()
        .await;

    if let Err(err) = commit_result {
        created_files.log_rollback().await;
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

    let result = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve default journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
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
    let journal_dir = state.storage().journal_dir(&journal);

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

async fn insert_files(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
    created: &DateTime<Utc>,
    dir: &JournalDir,
    files: &mut Vec<ResultFileEntry>,
) -> Result<CreatedFiles, error::Error> {
    let mut first = true;
    let mut params: db::ParamsVec<'_> = vec![entries_id, created];
    let mut query = String::from(
        "insert into file_entries ( \
            uid, \
            entries_id, \
            name, \
            mime_type, \
            mime_subtype, \
            mime_param, \
            created \
        ) values "
    );

    for entry in files.iter() {
        if first {
            first = false;
        } else {
            query.push_str(", ");
        }

        match &entry.inner {
            EntryFileForm::Server { uid, name, mime_type, mime_subtype, mime_param, .. } => {
                write!(
                    &mut query,
                    "(${}, $1, ${}, ${}, ${}, ${}, $2)",
                    db::push_param(&mut params, uid),
                    db::push_param(&mut params, name),
                    db::push_param(&mut params, mime_type),
                    db::push_param(&mut params, mime_subtype),
                    db::push_param(&mut params, mime_param),
                ).unwrap();
            }
        }
    }

    query.push_str(" returning id");

    tracing::debug!("file insert query: \"{query}\"");

    let results = match conn.query_raw(query.as_str(), params).await {
        Ok(r) => r.map(|stream| stream.map(|record|
            record.get::<usize, FileEntryId>(0)
        )),
        Err(err) => return Err(error::Error::context_source(
            "failed to insert files",
            err
        ))
    };

    futures::pin_mut!(results);

    let mut created_files = CreatedFiles::new();

    for file_entry in files {
        let Some(ins_result) = results.next().await else {
            return Err(error::Error::context(
                "less than expected number of ids returned from database"
            ));
        };

        match &mut file_entry.inner {
            EntryFileForm::Server { _id, .. } => {
                *_id = ins_result.context(
                    "failed to retrieve file entry id from insert"
                )?;

                let file_path = dir.file_path(_id);

                if let Err(err) = created_files.add(file_path).await {
                    created_files.log_rollback().await;

                    return Err(error::Error::context_source(
                        "failed to create file for journal entry",
                        err
                    ));
                }
            }
        }
    }

    Ok(created_files)
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
