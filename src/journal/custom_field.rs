use std::collections::HashSet;
use std::convert::TryFrom;

use bytes::BytesMut;
use chrono::{DateTime, Utc};
use futures::{StreamExt};
use postgres_types as pg_types;
use serde::{Serialize, Deserialize};

use crate::error::BoxDynError;
use crate::db::{self, GenericClient, PgError};
use crate::db::ids::{EntryId, CustomFieldId};

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
    pub value: T
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeValue<T> {
   pub low: T,
   pub high: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegerType {
   pub minimum: Option<i32>,
   pub maximum: Option<i32>
}

pub type IntegerValue = SimpleValue<i32>;

impl IntegerType {
    pub fn validate(
        &self,
        IntegerValue { value }: IntegerValue
    ) -> Result<IntegerValue, IntegerValue> {
        match (&self.minimum, &self.maximum) {
            (Some(min), Some(max)) if value >= *min && value <= *max => Ok(IntegerValue { value }),
            (Some(min), None) if value >= *min => Ok(IntegerValue { value }),
            (None, Some(max)) if value <= *max => Ok(IntegerValue { value }),
            (None, None) => Ok(IntegerValue { value }),
            _ => Err(IntegerValue { value }),
        }
    }

    pub fn make_value(&self) -> IntegerValue {
        IntegerValue {
            value: self.minimum.unwrap_or(0)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegerRangeType {
   pub minimum: Option<i32>,
   pub maximum: Option<i32>
}

pub type IntegerRangeValue = RangeValue<i32>;

impl IntegerRangeType {
    pub fn validate(
        &self,
        IntegerRangeValue { low, high }: IntegerRangeValue,
    ) -> Result<IntegerRangeValue, IntegerRangeValue> {
        match (&self.minimum, &self.maximum) {
            (Some(min), Some(max)) if low >= *min && low < high && high <= *max => Ok(IntegerRangeValue { low, high }),
            (Some(min), None) if low >= *min && low < high => Ok(IntegerRangeValue { low, high }),
            (None, Some(max)) if low < high && high <= *max => Ok(IntegerRangeValue { low, high }),
            (None, None) if low < high => Ok(IntegerRangeValue { low, high }),
            _ => Err(IntegerRangeValue { low, high }),
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
                high: *min + 10
            },
            (None, Some(max)) => IntegerRangeValue {
                low: *max - 10,
                high: *max
            },
            (None, None) => IntegerRangeValue {
                low: 0,
                high: 10
            },
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
   pub precision: i32
}

pub type FloatValue = SimpleValue<f32>;

impl FloatType {
    pub fn validate(
        &self,
        FloatValue { value }: FloatValue,
    ) -> Result<FloatValue, FloatValue> {
        match (&self.minimum, &self.maximum) {
            (Some(min), Some(max)) if value >= *min && value <= *max => Ok(FloatValue { value }),
            (Some(min), None) if value >= *min => Ok(FloatValue { value }),
            (None, Some(max)) if value <= *max => Ok(FloatValue { value }),
            (None, None) => Ok(FloatValue { value }),
            _ => Err(FloatValue { value }),
        }
    }

    pub fn make_value(&self) -> FloatValue {
        FloatValue {
            value: self.minimum.unwrap_or(0.0)
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
    pub precision: i32
}

pub type FloatRangeValue = RangeValue<f32>;

impl FloatRangeType {
    pub fn validate(
        &self,
        FloatRangeValue { low, high }: FloatRangeValue
    ) -> Result<FloatRangeValue, FloatRangeValue> {
        match (&self.minimum, &self.maximum) {
            (Some(min), Some(max)) if low >= *min && low < high && high <= *max => Ok(FloatRangeValue { low, high }),
            (Some(min), None) if low >= *min && low < high => Ok(FloatRangeValue { low, high }),
            (None, Some(max)) if low < high && high <= *max => Ok(FloatRangeValue { low, high }),
            (None, None) if low < high => Ok(FloatRangeValue { low, high }),
            _ => Err(FloatRangeValue { low, high }),
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
                high: *min + 10.0
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
    pub fn validate(
        &self,
        TimeValue { value }: TimeValue
    ) -> Result<TimeValue, TimeValue> {
        Ok(TimeValue { value })
    }

    pub fn make_value(&self) -> TimeValue {
        TimeValue {
            value: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRangeType {
    #[serde(default = "default_time_range_show_diff")]
    pub show_diff: bool,
}

pub type TimeRangeValue = RangeValue<DateTime<Utc>>;

impl TimeRangeType {
    pub fn validate(
        &self,
        TimeRangeValue { low, high }: TimeRangeValue,
    ) -> Result<TimeRangeValue, TimeRangeValue> {
        if low > high {
            Err(TimeRangeValue { low, high })
        } else {
            Ok(TimeRangeValue { low, high })
        }
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
            _ => Err(TypeMissMatch)
        }
    }
}

impl pg_types::ToSql for IntegerValue {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
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
            _ => Err(TypeMissMatch)
        }
    }
}

impl pg_types::ToSql for IntegerRangeValue {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
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
            _ => Err(TypeMissMatch)
        }
    }
}

impl pg_types::ToSql for FloatValue {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
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
            _ => Err(TypeMissMatch)
        }
    }
}

impl pg_types::ToSql for FloatRangeValue {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
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
            _ => Err(TypeMissMatch)
        }
    }
}

impl pg_types::ToSql for TimeValue {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
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
            _ => Err(TypeMissMatch)
        }
    }
}

impl pg_types::ToSql for TimeRangeValue {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
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

pub async fn retrieve_known_entry_ids(
    conn: &impl GenericClient,
    entries_id: &EntryId,
) -> Result<HashSet<CustomFieldId>, PgError> {
    let params: db::ParamsArray<'_, 1> = [entries_id];

    let stream = conn.query_raw(
        "\
        select custom_field_entries.custom_fields_id \
        from custom_field_entries \
        where custom_field_entries.entries_id = $1",
        params
    ).await?;

    futures::pin_mut!(stream);

    let mut rtn = HashSet::new();

    while let Some(try_row) = stream.next().await {
        let row = try_row?;

        rtn.insert(row.get(0));
    }

    Ok(rtn)
}

#[cfg(test)]
mod test {
    use super::*;

    use chrono::{Utc, Duration};

    const INT: IntegerType = IntegerType {
        minimum: Some(1),
        maximum: Some(10),
    };
    const INT_LOW: IntegerType = IntegerType {
        minimum: Some(1),
        maximum: None,
    };
    const INT_HIGH: IntegerType = IntegerType {
        minimum: None,
        maximum: Some(10),
    };
    const INT_NO_LIMIT: IntegerType = IntegerType {
        minimum: None,
        maximum: None,
    };

    const INT_RANGE: IntegerRangeType = IntegerRangeType {
        minimum: Some(1),
        maximum: Some(10),
    };
    const INT_RANGE_LOW: IntegerRangeType = IntegerRangeType {
        minimum: Some(1),
        maximum: None,
    };
    const INT_RANGE_HIGH: IntegerRangeType = IntegerRangeType {
        minimum: None,
        maximum: Some(10),
    };
    const INT_RANGE_NO_LIMIT: IntegerRangeType = IntegerRangeType {
        minimum: None,
        maximum: None,
    };

    const FLOAT: FloatType = FloatType {
        minimum: Some(1.0),
        maximum: Some(10.0),
        step: 0.1,
        precision: 2,
    };
    const FLOAT_LOW: FloatType = FloatType {
        minimum: Some(1.0),
        maximum: None,
        step: 0.1,
        precision: 2,
    };
    const FLOAT_HIGH: FloatType = FloatType {
        minimum: None,
        maximum: Some(10.0),
        step: 0.1,
        precision: 2,
    };
    const FLOAT_NO_LIMIT: FloatType = FloatType {
        minimum: None,
        maximum: None,
        step: 0.1,
        precision: 2,
    };

    const FLOAT_RANGE: FloatRangeType = FloatRangeType {
        minimum: Some(1.0),
        maximum: Some(10.0),
        step: 0.1,
        precision: 2,
    };
    const FLOAT_RANGE_LOW: FloatRangeType = FloatRangeType {
        minimum: Some(1.0),
        maximum: None,
        step: 0.1,
        precision: 2,
    };
    const FLOAT_RANGE_HIGH: FloatRangeType = FloatRangeType {
        minimum: None,
        maximum: Some(10.0),
        step: 0.1,
        precision: 2,
    };
    const FLOAT_RANGE_NO_LIMIT: FloatRangeType = FloatRangeType {
        minimum: None,
        maximum: None,
        step: 0.1,
        precision: 2,
    };

    const TIME: TimeType = TimeType {
        as_12hr: false
    };
    const TIME_RANGE: TimeRangeType = TimeRangeType {
        show_diff: false,
        as_12hr: false
    };

    #[test]
    fn integer() {
        let given = IntegerValue { value: 5 };
        let given_low = IntegerValue { value: 1 };
        let given_high = IntegerValue { value: 10 };

        assert!(INT.validate(given).is_ok());
        assert!(INT.validate(given_low).is_ok());
        assert!(INT.validate(given_high).is_ok());
    }

    #[test]
    fn integer_low() {
        let given = IntegerValue { value: 5 };
        let given_low = IntegerValue { value: 1 };
        let given_high = IntegerValue { value: i32::MAX };

        assert!(INT_LOW.validate(given).is_ok());
        assert!(INT_LOW.validate(given_low).is_ok());
        assert!(INT_LOW.validate(given_high).is_ok());
    }

    #[test]
    fn integer_high() {
        let given = IntegerValue { value: 5 };
        let given_low = IntegerValue { value: i32::MIN };
        let given_high = IntegerValue { value: 10 };

        assert!(INT_HIGH.validate(given).is_ok());
        assert!(INT_HIGH.validate(given_low).is_ok());
        assert!(INT_HIGH.validate(given_high).is_ok());
    }

    #[test]
    fn integer_no_limit() {
        let given = IntegerValue { value: 5 };
        let given_low = IntegerValue { value: i32::MIN };
        let given_high = IntegerValue { value: i32::MAX };

        assert!(INT_NO_LIMIT.validate(given).is_ok());
        assert!(INT_NO_LIMIT.validate(given_low).is_ok());
        assert!(INT_NO_LIMIT.validate(given_high).is_ok());
    }

    #[test]
    fn integer_range() {
        let given = IntegerRangeValue { low: 3, high: 7 };
        let given_low = IntegerRangeValue { low: 1, high: 7 };
        let given_high = IntegerRangeValue { low: 3, high: 10 };
        let given_bounds = IntegerRangeValue { low: 1, high: 10 };

        assert!(INT_RANGE.validate(given).is_ok());
        assert!(INT_RANGE.validate(given_low).is_ok());
        assert!(INT_RANGE.validate(given_high).is_ok());
        assert!(INT_RANGE.validate(given_bounds).is_ok());
    }

    #[test]
    fn integer_range_low() {
        let given = IntegerRangeValue { low: 3, high: 7 };
        let given_low = IntegerRangeValue { low: 1, high: i32::MAX };
        let given_high = IntegerRangeValue { low: 3, high: i32::MAX };

        assert!(INT_RANGE_LOW.validate(given).is_ok());
        assert!(INT_RANGE_LOW.validate(given_low).is_ok());
        assert!(INT_RANGE_LOW.validate(given_high).is_ok());
    }

    #[test]
    fn integer_range_high() {
        let given = IntegerRangeValue { low: 3, high: 7 };
        let given_low = IntegerRangeValue { low: i32::MIN, high: 7 };
        let given_high = IntegerRangeValue { low: i32::MIN, high: 10 };

        assert!(INT_RANGE_HIGH.validate(given).is_ok());
        assert!(INT_RANGE_HIGH.validate(given_low).is_ok());
        assert!(INT_RANGE_HIGH.validate(given_high).is_ok());
    }

    #[test]
    fn integer_range_no_limit() {
        let given = IntegerRangeValue { low: 3, high: 7 };
        let given_bounds = IntegerRangeValue { low: i32::MIN, high: i32::MAX };

        assert!(INT_RANGE_NO_LIMIT.validate(given).is_ok());
        assert!(INT_RANGE_NO_LIMIT.validate(given_bounds).is_ok());
    }

    #[test]
    fn float() {
        let given = FloatValue { value: 5.0 };
        let given_low = FloatValue { value: 1.0 };
        let given_high = FloatValue { value: 10.0 };

        assert!(FLOAT.validate(given).is_ok());
        assert!(FLOAT.validate(given_low).is_ok());
        assert!(FLOAT.validate(given_high).is_ok());
    }

    #[test]
    fn float_low() {
        let given = FloatValue { value: 5.0 };
        let given_low = FloatValue { value: 1.0 };
        let given_high = FloatValue { value: f32::MAX };

        assert!(FLOAT_LOW.validate(given).is_ok());
        assert!(FLOAT_LOW.validate(given_low).is_ok());
        assert!(FLOAT_LOW.validate(given_high).is_ok());
    }

    #[test]
    fn float_high() {
        let given = FloatValue { value: 5.0 };
        let given_low = FloatValue { value: f32::MIN };
        let given_high = FloatValue { value: 10.0 };

        assert!(FLOAT_HIGH.validate(given).is_ok());
        assert!(FLOAT_HIGH.validate(given_low).is_ok());
        assert!(FLOAT_HIGH.validate(given_high).is_ok());
    }

    #[test]
    fn float_no_limit() {
        let given = FloatValue { value: 5.0 };
        let given_low = FloatValue { value: f32::MIN };
        let given_high = FloatValue { value: f32::MAX };

        assert!(FLOAT_NO_LIMIT.validate(given).is_ok());
        assert!(FLOAT_NO_LIMIT.validate(given_low).is_ok());
        assert!(FLOAT_NO_LIMIT.validate(given_high).is_ok());
    }

    #[test]
    fn float_range() {
        let given = FloatRangeValue { low: 3.0, high: 7.0 };
        let given_low = FloatRangeValue { low: 1.0, high: 7.0 };
        let given_high = FloatRangeValue { low: 3.0, high: 10.0 };
        let given_bounds = FloatRangeValue { low: 1.0, high: 10.0 };

        assert!(FLOAT_RANGE.validate(given).is_ok());
        assert!(FLOAT_RANGE.validate(given_low).is_ok());
        assert!(FLOAT_RANGE.validate(given_high).is_ok());
        assert!(FLOAT_RANGE.validate(given_bounds).is_ok());
    }

    #[test]
    fn float_range_low() {
        let given = FloatRangeValue { low: 3.0, high: 7.0 };
        let given_low = FloatRangeValue { low: 1.0, high: f32::MAX };
        let given_high = FloatRangeValue { low: 3.0, high: f32::MAX };

        assert!(FLOAT_RANGE_LOW.validate(given).is_ok());
        assert!(FLOAT_RANGE_LOW.validate(given_low).is_ok());
        assert!(FLOAT_RANGE_LOW.validate(given_high).is_ok());
    }

    #[test]
    fn float_range_high() {
        let given = FloatRangeValue { low: 3.0, high: 7.0 };
        let given_low = FloatRangeValue { low: f32::MIN, high: 7.0 };
        let given_high = FloatRangeValue { low: f32::MIN, high: 10.0 };

        assert!(FLOAT_RANGE_HIGH.validate(given).is_ok());
        assert!(FLOAT_RANGE_HIGH.validate(given_low).is_ok());
        assert!(FLOAT_RANGE_HIGH.validate(given_high).is_ok());
    }

    #[test]
    fn float_range_no_limit() {
        let given = FloatRangeValue { low: 3.0, high: 7.0 };
        let given_bounds = FloatRangeValue { low: f32::MIN, high: f32::MAX };

        assert!(FLOAT_RANGE_NO_LIMIT.validate(given).is_ok());
        assert!(FLOAT_RANGE_NO_LIMIT.validate(given_bounds).is_ok());
    }

    #[test]
    fn time() {
        let given = TimeValue { value: Utc::now() };

        assert!(TIME.validate(given).is_ok());
    }

    #[test]
    fn time_range() {
        let given = TimeRangeValue {
            low: Utc::now(),
            high: Utc::now() + Duration::new(10, 0).unwrap(),
        };

        assert!(TIME_RANGE.validate(given).is_ok());
    }
}
