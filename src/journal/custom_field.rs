use bytes::BytesMut;
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use postgres_types as pg_types;
use serde::{Serialize, Deserialize};

use crate::error::BoxDynError;
use crate::db::{self, GenericClient, PgError};
use crate::db::ids::{EntryId, CustomFieldId};

fn default_time_range_show_diff() -> bool {
    false
}

fn default_as_12hr() -> bool {
    false
}

fn default_step() -> f64 {
    0.01
}

fn default_precision() -> i32 {
    2
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Type {
    Integer {
        minimum: Option<i32>,
        maximum: Option<i32>
    },
    IntegerRange {
        minimum: Option<i32>,
        maximum: Option<i32>
    },

    Float {
        minimum: Option<f32>,
        maximum: Option<f32>,
        #[serde(default = "default_step")]
        step: f64,
        #[serde(default = "default_precision")]
        precision: i32
    },
    FloatRange {
        minimum: Option<f32>,
        maximum: Option<f32>,
        #[serde(default = "default_step")]
        step: f64,
        #[serde(default = "default_precision")]
        precision: i32
    },

    Time {
        #[serde(default = "default_as_12hr")]
        as_12hr: bool
    },
    TimeRange {
        #[serde(default = "default_time_range_show_diff")]
        show_diff: bool,

        #[serde(default = "default_as_12hr")]
        as_12hr: bool
    },
}

impl pg_types::ToSql for Type {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        let wrapper: pg_types::Json<&Self> = pg_types::Json(self);

        wrapper.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <pg_types::Json<Self> as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl<'a> pg_types::FromSql<'a> for Type {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let parsed: pg_types::Json<Self> = pg_types::Json::from_sql(ty, raw)?;

        Ok(parsed.0)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <pg_types::Json<Self> as pg_types::FromSql>::accepts(ty)
    }
}

#[derive(Debug)]
pub struct Entry {
    pub custom_fields_id: CustomFieldId,
    pub entries_id: EntryId,
    pub value: Value,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Value {
    Integer {
        value: i32
    },
    IntegerRange {
        low: i32,
        high: i32
    },

    Float {
        value: f32
    },
    FloatRange {
        low: f32,
        high: f32
    },

    Time {
        //#[serde(with = "ts_seconds")]
        value: DateTime<Utc>
    },
    TimeRange {
        //#[serde(with = "ts_seconds")]
        low: DateTime<Utc>,

        //#[serde(with = "ts_seconds")]
        high: DateTime<Utc>
    },
}

impl Entry {
    pub async fn retrieve_entries_id_stream(
        conn: &impl GenericClient,
        entries_id: &EntryId
    ) -> Result<impl Stream<Item = Result<Self, PgError>>, PgError> {
        let params: db::ParamsArray<'_, 1> = [entries_id];

        Ok(conn.query_raw(
            "\
            select custom_field_entries.custom_fields_id, \
                   custom_field_entries.entries_id, \
                   custom_field_entries.value, \
                   custom_field_entries.created, \
                   custom_field_entries.updated \
            from custom_field_entries \
            where custom_field_entries.entries_id = $1",
            params
        )
            .await?
            .map(|stream| stream.map(|row| Self {
                custom_fields_id: row.get(0),
                entries_id: row.get(1),
                value: row.get(2),
                created: row.get(3),
                updated: row.get(4),
            })))
    }
}

impl pg_types::ToSql for Value {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        let wrapper: pg_types::Json<&Self> = pg_types::Json(self);

        wrapper.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <pg_types::Json<Self> as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl<'a> pg_types::FromSql<'a> for Value {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let parsed: pg_types::Json<Self> = pg_types::Json::from_sql(ty, raw)?;

        Ok(parsed.0)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <pg_types::Json<Self> as pg_types::FromSql>::accepts(ty)
    }
}
