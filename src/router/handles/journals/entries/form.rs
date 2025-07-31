use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use futures::{Stream, StreamExt};
use serde::Serialize;

use crate::db;
use crate::db::ids::{CustomFieldId, EntryId, EntryUid, FileEntryId, FileEntryUid, JournalId};
use crate::journal::{custom_field, FileEntry, ReceivedFile, RequestedFile};

#[derive(Debug, Serialize)]
pub struct EntryForm<FileT = EntryFileForm> {
    pub id: Option<EntryId>,
    pub uid: Option<EntryUid>,
    pub date: NaiveDate,
    pub title: Option<String>,
    pub contents: Option<String>,
    pub tags: Vec<EntryTagForm>,
    pub files: Vec<FileT>,
    pub custom_fields: Vec<EntryCustomFieldForm>,
}

impl EntryForm {
    pub async fn blank(
        conn: &impl db::GenericClient,
        journals_id: &JournalId,
    ) -> Result<Self, db::PgError> {
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
    ) -> Result<Option<Self>, db::PgError> {
        let maybe = conn
            .query_opt(
                "\
            select entries.id, \
                   entries.uid, \
                   entries.entry_date, \
                   entries.title, \
                   entries.contents \
            from entries \
            where entries.journals_id = $1 and \
                  entries.id = $2",
                &[journals_id, entries_id],
            )
            .await?;

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
    pub key: String,
    pub value: Option<String>,
}

impl EntryTagForm {
    pub async fn retrieve_entry_stream(
        conn: &impl db::GenericClient,
        entries_id: &EntryId,
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [entries_id];

        let stream = conn
            .query_raw(
                "\
            select entry_tags.key, \
                   entry_tags.value \
            from entry_tags \
            where entry_tags.entries_id = $1",
                params,
            )
            .await?;

        Ok(stream.map(|result| {
            result.map(|record| Self {
                key: record.get(0),
                value: record.get(1),
            })
        }))
    }

    pub async fn retrieve_entry(
        conn: &impl db::GenericClient,
        entries_id: &EntryId,
    ) -> Result<Vec<Self>, db::PgError> {
        let stream = Self::retrieve_entry_stream(conn, entries_id).await?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_record) = stream.next().await {
            rtn.push(try_record?);
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
    ) -> Result<Vec<Self>, db::PgError> {
        let stream = Self::retrieve_entry_stream(conn, entries_id).await?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_record) = stream.next().await {
            rtn.push(try_record?);
        }

        Ok(rtn)
    }

    pub async fn retrieve_entry_map(
        conn: &impl db::GenericClient,
        entries_id: &EntryId,
    ) -> Result<HashMap<FileEntryId, Self>, db::PgError> {
        let stream = Self::retrieve_entry_stream(conn, entries_id).await?;

        futures::pin_mut!(stream);

        let mut rtn = HashMap::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record?;

            rtn.insert(*record.id(), record);
        }

        Ok(rtn)
    }

    pub fn id(&self) -> &FileEntryId {
        match self {
            Self::Requested { _id, .. } | Self::Received { _id, .. } => _id,
        }
    }

    pub fn is_received(&self) -> bool {
        match self {
            Self::Received { .. } => true,
            _ => false,
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
        }
    }
}

impl From<RequestedFile> for EntryFileForm {
    fn from(given: RequestedFile) -> Self {
        Self::Requested {
            _id: given.id,
            uid: given.uid,
            name: given.name,
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
    pub _id: CustomFieldId,
    pub enabled: bool,
    pub order: i32,
    pub name: String,
    pub description: Option<String>,
    pub config: T,
    pub value: V,
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
    pub fn get_record(
        _id: CustomFieldId,
        order: i32,
        name: String,
        description: Option<String>,
        ty: custom_field::Type,
        v: Option<custom_field::Value>,
    ) -> Self {
        match ty {
            custom_field::Type::Integer(config) => {
                let mapped = v.map(|exists| {
                    exists
                        .try_into()
                        .expect("failed to convert custom field entry into integer value")
                });

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
                let mapped = v.map(|exists| {
                    exists
                        .try_into()
                        .expect("failed to convert custom field entry into integer range value")
                });

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
                let mapped = v.map(|exists| {
                    exists
                        .try_into()
                        .expect("failed to convert custom field entry into float value")
                });

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
                let mapped = v.map(|exists| {
                    exists
                        .try_into()
                        .expect("failed to convert custom field entry into float range value")
                });

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
                let mapped = v.map(|exists| {
                    exists
                        .try_into()
                        .expect("failed to convert custom field entry into time value")
                });

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
                let mapped = v.map(|exists| {
                    exists
                        .try_into()
                        .expect("failed to convert custom field entry into time range value")
                });

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

    pub async fn retrieve_empty(
        conn: &impl db::GenericClient,
        journals_id: &JournalId,
    ) -> Result<Vec<Self>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [journals_id];
        let stream = conn
            .query_raw(
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
                params,
            )
            .await?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_row) = stream.next().await {
            let row = try_row?;

            rtn.push(Self::get_record(
                row.get(0),
                row.get(4),
                row.get(1),
                row.get(3),
                row.get(2),
                None,
            ));
        }

        Ok(rtn)
    }

    pub async fn retrieve_entry_stream(
        conn: &impl db::GenericClient,
        journals_id: &JournalId,
        entries_id: &EntryId,
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 2> = [journals_id, entries_id];
        let stream = conn
            .query_raw(
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
                params,
            )
            .await?;

        Ok(stream.map(|try_record| {
            try_record.map(|row| {
                Self::get_record(
                    row.get(0),
                    row.get(4),
                    row.get(1),
                    row.get(3),
                    row.get(2),
                    row.get(5),
                )
            })
        }))
    }

    pub async fn retrieve_entry(
        conn: &impl db::GenericClient,
        journals_id: &JournalId,
        entries_id: &EntryId,
    ) -> Result<Vec<Self>, db::PgError> {
        let stream = Self::retrieve_entry_stream(conn, journals_id, entries_id).await?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_record) = stream.next().await {
            rtn.push(try_record?);
        }

        Ok(rtn)
    }
}
