use std::collections::HashSet;
use std::convert::TryFrom;

use bytes::BytesMut;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use postgres_types as pg_types;
use serde::{Deserialize, Serialize};

use crate::db::ids::{CustomFieldId, EntryId};
use crate::db::{self, GenericClient, PgError};
use crate::error::BoxDynError;

fn default_time_range_show_diff() -> bool {
    false
}

fn default_step() -> f32 {
    0.01
}

fn default_precision() -> i32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleValue<T> {
    pub value: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeValue<T> {
    pub low: T,
    pub high: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegerType {
    pub minimum: Option<i32>,
    pub maximum: Option<i32>,
}

pub type IntegerValue = SimpleValue<i32>;

impl IntegerType {
    pub fn validate(&self, IntegerValue { value }: &IntegerValue) -> bool {
        match (&self.minimum, &self.maximum) {
            (Some(min), Some(max)) => *value >= *min && *value <= *max,
            (Some(min), None) => *value >= *min,
            (None, Some(max)) => *value <= *max,
            (None, None) => true,
        }
    }

    pub fn make_value(&self) -> IntegerValue {
        IntegerValue {
            value: self.minimum.unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegerRangeType {
    pub minimum: Option<i32>,
    pub maximum: Option<i32>,
}

pub type IntegerRangeValue = RangeValue<i32>;

impl IntegerRangeType {
    pub fn validate(&self, IntegerRangeValue { low, high }: &IntegerRangeValue) -> bool {
        match (&self.minimum, &self.maximum) {
            (Some(min), Some(max)) => *low >= *min && *low < *high && *high <= *max,
            (Some(min), None) => *low >= *min && *low < *high,
            (None, Some(max)) => *low < *high && *high <= *max,
            (None, None) => *low < *high,
        }
    }

    pub fn make_value(&self) -> IntegerRangeValue {
        match (&self.minimum, &self.maximum) {
            (Some(min), Some(max)) => IntegerRangeValue {
                low: *min,
                high: *max,
            },
            (Some(min), None) => IntegerRangeValue {
                low: *min,
                high: *min + 10,
            },
            (None, Some(max)) => IntegerRangeValue {
                low: *max - 10,
                high: *max,
            },
            (None, None) => IntegerRangeValue { low: 0, high: 10 },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatType {
    pub minimum: Option<f32>,
    pub maximum: Option<f32>,

    #[serde(default = "default_step")]
    pub step: f32,

    #[serde(default = "default_precision")]
    pub precision: i32,
}

pub type FloatValue = SimpleValue<f32>;

impl FloatType {
    pub fn validate(&self, FloatValue { value }: &FloatValue) -> bool {
        match (&self.minimum, &self.maximum) {
            (Some(min), Some(max)) => *value >= *min && *value <= *max,
            (Some(min), None) => *value >= *min,
            (None, Some(max)) => *value <= *max,
            (None, None) => true,
        }
    }

    pub fn make_value(&self) -> FloatValue {
        FloatValue {
            value: self.minimum.unwrap_or(0.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatRangeType {
    pub minimum: Option<f32>,
    pub maximum: Option<f32>,

    #[serde(default = "default_step")]
    pub step: f32,

    #[serde(default = "default_precision")]
    pub precision: i32,
}

pub type FloatRangeValue = RangeValue<f32>;

impl FloatRangeType {
    pub fn validate(&self, FloatRangeValue { low, high }: &FloatRangeValue) -> bool {
        match (&self.minimum, &self.maximum) {
            (Some(min), Some(max)) => *low >= *min && *low < *high && *high <= *max,
            (Some(min), None) => *low >= *min && *low < *high,
            (None, Some(max)) => *low < *high && *high <= *max,
            (None, None) => *low < *high,
        }
    }

    pub fn make_value(&self) -> FloatRangeValue {
        match (&self.minimum, &self.maximum) {
            (Some(min), Some(max)) => FloatRangeValue {
                low: *min,
                high: *max,
            },
            (Some(min), None) => FloatRangeValue {
                low: *min,
                high: *min + 10.0,
            },
            (None, Some(max)) => FloatRangeValue {
                low: *max - 10.0,
                high: *max,
            },
            (None, None) => FloatRangeValue {
                low: 0.0,
                high: 10.0,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeType {}

pub type TimeValue = SimpleValue<DateTime<Utc>>;

impl TimeType {
    pub fn validate(&self, TimeValue { .. }: &TimeValue) -> bool {
        true
    }

    pub fn make_value(&self) -> TimeValue {
        TimeValue { value: Utc::now() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRangeType {
    #[serde(default = "default_time_range_show_diff")]
    pub show_diff: bool,
}

pub type TimeRangeValue = RangeValue<DateTime<Utc>>;

impl TimeRangeType {
    pub fn validate(&self, TimeRangeValue { low, high }: &TimeRangeValue) -> bool {
        *low < *high
    }

    pub fn make_value(&self) -> TimeRangeValue {
        let now = Utc::now();
        let dur = chrono::Duration::hours(1);

        TimeRangeValue {
            low: now - dur,
            high: now + dur,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Type {
    Integer(IntegerType),
    IntegerRange(IntegerRangeType),

    Float(FloatType),
    FloatRange(FloatRangeType),

    Time(TimeType),
    TimeRange(TimeRangeType),
}

pub enum ValidationError {
    Mismatched,
    Invalid,
}

impl Type {
    pub fn validate(&self, value: &Value) -> Result<(), ValidationError> {
        match self {
            Self::Integer(ty) => match value {
                Value::Integer(check) => {
                    if !ty.validate(check) {
                        Err(ValidationError::Invalid)
                    } else {
                        Ok(())
                    }
                }
                _ => Err(ValidationError::Mismatched),
            },
            Self::IntegerRange(ty) => match value {
                Value::IntegerRange(check) => {
                    if !ty.validate(check) {
                        Err(ValidationError::Invalid)
                    } else {
                        Ok(())
                    }
                }
                _ => Err(ValidationError::Mismatched),
            },
            Self::Float(ty) => match value {
                Value::Float(check) => {
                    if !ty.validate(check) {
                        Err(ValidationError::Invalid)
                    } else {
                        Ok(())
                    }
                }
                _ => Err(ValidationError::Mismatched),
            },
            Self::FloatRange(ty) => match value {
                Value::FloatRange(check) => {
                    if !ty.validate(check) {
                        Err(ValidationError::Invalid)
                    } else {
                        Ok(())
                    }
                }
                _ => Err(ValidationError::Mismatched),
            },
            Self::Time(ty) => match value {
                Value::Time(check) => {
                    if !ty.validate(check) {
                        Err(ValidationError::Invalid)
                    } else {
                        Ok(())
                    }
                }
                _ => Err(ValidationError::Mismatched),
            },
            Self::TimeRange(ty) => match value {
                Value::TimeRange(check) => {
                    if !ty.validate(check) {
                        Err(ValidationError::Invalid)
                    } else {
                        Ok(())
                    }
                }
                _ => Err(ValidationError::Mismatched),
            },
        }
    }
}

impl From<IntegerType> for Type {
    fn from(v: IntegerType) -> Self {
        Type::Integer(v)
    }
}

impl From<IntegerRangeType> for Type {
    fn from(v: IntegerRangeType) -> Self {
        Type::IntegerRange(v)
    }
}

impl From<FloatType> for Type {
    fn from(v: FloatType) -> Self {
        Type::Float(v)
    }
}

impl From<FloatRangeType> for Type {
    fn from(v: FloatRangeType) -> Self {
        Type::FloatRange(v)
    }
}

impl From<TimeType> for Type {
    fn from(v: TimeType) -> Self {
        Type::Time(v)
    }
}

impl From<TimeRangeType> for Type {
    fn from(v: TimeRangeType) -> Self {
        Type::TimeRange(v)
    }
}

impl pg_types::ToSql for Type {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
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

/*
#[derive(Debug)]
pub struct Entry {
    pub custom_fields_id: CustomFieldId,
    pub entries_id: EntryId,
    pub value: Value,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}
*/

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Value {
    Integer(IntegerValue),
    IntegerRange(IntegerRangeValue),

    Float(FloatValue),
    FloatRange(FloatRangeValue),

    Time(TimeValue),
    TimeRange(TimeRangeValue),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ValueRef<'a> {
    Integer(&'a IntegerValue),
    IntegerRange(&'a IntegerRangeValue),
    Float(&'a FloatValue),
    FloatRange(&'a FloatRangeValue),
    Time(&'a TimeValue),
    TimeRange(&'a TimeRangeValue),
}

#[derive(Debug, thiserror::Error)]
#[error("the underlying type does not match the requested type")]
pub struct TypeMissMatch;

impl From<IntegerValue> for Value {
    fn from(v: IntegerValue) -> Self {
        Value::Integer(v)
    }
}

impl TryFrom<Value> for IntegerValue {
    type Error = TypeMissMatch;

    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::Integer(v) => Ok(v),
            _ => Err(TypeMissMatch),
        }
    }
}

impl pg_types::ToSql for IntegerValue {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
        let value_ref = ValueRef::Integer(self);
        let wrapper: pg_types::Json<&ValueRef<'_>> = pg_types::Json(&value_ref);

        wrapper.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <pg_types::Json<ValueRef<'_>> as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl From<IntegerRangeValue> for Value {
    fn from(v: IntegerRangeValue) -> Self {
        Value::IntegerRange(v)
    }
}

impl TryFrom<Value> for IntegerRangeValue {
    type Error = TypeMissMatch;

    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::IntegerRange(v) => Ok(v),
            _ => Err(TypeMissMatch),
        }
    }
}

impl pg_types::ToSql for IntegerRangeValue {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
        let value_ref = ValueRef::IntegerRange(self);
        let wrapper: pg_types::Json<&ValueRef<'_>> = pg_types::Json(&value_ref);

        wrapper.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <pg_types::Json<ValueRef<'_>> as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl From<FloatValue> for Value {
    fn from(v: FloatValue) -> Self {
        Value::Float(v)
    }
}

impl TryFrom<Value> for FloatValue {
    type Error = TypeMissMatch;

    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::Float(v) => Ok(v),
            _ => Err(TypeMissMatch),
        }
    }
}

impl pg_types::ToSql for FloatValue {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
        let value_ref = ValueRef::Float(self);
        let wrapper: pg_types::Json<&ValueRef<'_>> = pg_types::Json(&value_ref);

        wrapper.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <pg_types::Json<ValueRef<'_>> as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl From<FloatRangeValue> for Value {
    fn from(v: FloatRangeValue) -> Self {
        Value::FloatRange(v)
    }
}

impl TryFrom<Value> for FloatRangeValue {
    type Error = TypeMissMatch;

    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::FloatRange(v) => Ok(v),
            _ => Err(TypeMissMatch),
        }
    }
}

impl pg_types::ToSql for FloatRangeValue {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
        let value_ref = ValueRef::FloatRange(self);
        let wrapper: pg_types::Json<&ValueRef<'_>> = pg_types::Json(&value_ref);

        wrapper.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <pg_types::Json<ValueRef<'_>> as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl From<TimeValue> for Value {
    fn from(v: TimeValue) -> Self {
        Value::Time(v)
    }
}

impl TryFrom<Value> for TimeValue {
    type Error = TypeMissMatch;

    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::Time(v) => Ok(v),
            _ => Err(TypeMissMatch),
        }
    }
}

impl pg_types::ToSql for TimeValue {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
        let value_ref = ValueRef::Time(self);
        let wrapper: pg_types::Json<&ValueRef<'_>> = pg_types::Json(&value_ref);

        wrapper.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <pg_types::Json<ValueRef<'_>> as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl From<TimeRangeValue> for Value {
    fn from(v: TimeRangeValue) -> Self {
        Value::TimeRange(v)
    }
}

impl TryFrom<Value> for TimeRangeValue {
    type Error = TypeMissMatch;

    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::TimeRange(v) => Ok(v),
            _ => Err(TypeMissMatch),
        }
    }
}

impl pg_types::ToSql for TimeRangeValue {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
        let value_ref = ValueRef::TimeRange(self);
        let wrapper: pg_types::Json<&ValueRef<'_>> = pg_types::Json(&value_ref);

        wrapper.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <pg_types::Json<ValueRef<'_>> as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl pg_types::ToSql for Value {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
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

pub async fn retrieve_known_entry_ids(
    conn: &impl GenericClient,
    entries_id: &EntryId,
) -> Result<HashSet<CustomFieldId>, PgError> {
    let params: db::ParamsArray<'_, 1> = [entries_id];

    let stream = conn
        .query_raw(
            "\
        select custom_field_entries.custom_fields_id \
        from custom_field_entries \
        where custom_field_entries.entries_id = $1",
            params,
        )
        .await?;

    futures::pin_mut!(stream);

    let mut rtn = HashSet::new();

    while let Some(try_row) = stream.next().await {
        let row = try_row?;

        rtn.insert(row.get(0));
    }

    Ok(rtn)
}
