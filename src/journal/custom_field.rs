use std::collections::HashMap;

use bytes::BytesMut;
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use postgres_types as pg_types;
use serde::{Serialize, Deserialize};

use crate::error::BoxDynError;
use crate::db::{self, GenericClient, PgError};
use crate::db::ids::{JournalId, EntryId, CustomFieldId};

fn default_time_range_show_diff() -> bool {
    false
}

fn default_as_12hr() -> bool {
    false
}

fn default_step() -> f32 {
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
        step: f32,
        #[serde(default = "default_precision")]
        precision: i32
    },
    FloatRange {
        minimum: Option<f32>,
        maximum: Option<f32>,
        #[serde(default = "default_step")]
        step: f32,
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

impl Type {
    pub async fn retrieve_journal_map(
        conn: &impl db::GenericClient,
        journals_id: &JournalId,
    ) -> Result<HashMap<CustomFieldId, Self>, PgError> {
        let params: db::ParamsArray<'_, 1> = [journals_id];

        let stream = conn.query_raw(
            "\
            select custom_fields.id, \
                   custom_fields.config \
            from custom_fields \
            where custom_fields.journals_id = $1",
            params
        ).await?;

        futures::pin_mut!(stream);

        let mut rtn = HashMap::new();

        while let Some(result) = stream.next().await {
            let row = result?;

            rtn.insert(row.get(0), row.get(1));
        }

        Ok(rtn)
    }

    pub fn validate(&self, given: Value) -> Result<Value, Value> {
        match self {
            Type::Integer {
                minimum,
                maximum
            } => match given {
                Value::Integer { value } => match (minimum, maximum) {
                    (Some(min), Some(max)) if value >= *min && value <= *max => Ok(Value::Integer { value }),
                    (Some(min), None) if value >= *min => Ok(Value::Integer { value }),
                    (None, Some(max)) if value <= *max => Ok(Value::Integer { value }),
                    (None, None) => Ok(Value::Integer { value }),
                    _ => Err(Value::Integer { value }),
                }
                _ => Err(given),
            }
            Type::IntegerRange {
                minimum,
                maximum,
            } => match given {
                Value::IntegerRange { low, high } => match (minimum, maximum) {
                    (Some(min), Some(max)) if low >= *min && low < high && high <= *max => Ok(Value::IntegerRange { low, high }),
                    (Some(min), None) if low >= *min && low < high => Ok(Value::IntegerRange { low, high }),
                    (None, Some(max)) if low < high && high <= *max => Ok(Value::IntegerRange { low, high }),
                    (None, None) if low < high => Ok(Value::IntegerRange { low, high }),
                    _ => Err(Value::IntegerRange { low, high }),
                }
                _ => Err(given),
            }
            Type::Float {
                minimum,
                maximum,
                ..
            } => match given {
                Value::Float { value } => match (minimum, maximum) {
                    (Some(min), Some(max)) if value >= *min && value <= *max => Ok(Value::Float { value }),
                    (Some(min), None) if value >= *min => Ok(Value::Float { value }),
                    (None, Some(max)) if value <= *max => Ok(Value::Float { value }),
                    (None, None) => Ok(Value::Float { value }),
                    _ => Err(Value::Float { value }),
                }
                _ => Err(given),
            }
            Type::FloatRange {
                minimum,
                maximum,
                ..
            } => match given {
                Value::FloatRange { low, high } => match (minimum, maximum) {
                    (Some(min), Some(max)) if low >= *min && low < high && high <= *max => Ok(Value::FloatRange { low, high }),
                    (Some(min), None) if low >= *min && low < high => Ok(Value::FloatRange { low, high }),
                    (None, Some(max)) if low < high && high <= *max => Ok(Value::FloatRange { low, high }),
                    (None, None) if low < high => Ok(Value::FloatRange { low, high }),
                    _ => Err(Value::FloatRange { low, high }),
                }
                _ => Err(given),
            }
            Type::Time {..} => match given {
                Value::Time { value } => Ok(Value::Time { value }),
                _ => Err(given),
            }
            Type::TimeRange {..} => match given {
                Value::TimeRange { low, high } if low < high => Ok(Value::TimeRange { low, high }),
                _ => Err(given),
            }
        }
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
        value: DateTime<Utc>
    },
    TimeRange {
        low: DateTime<Utc>,
        high: DateTime<Utc>
    },
}

impl Entry {
    pub async fn retrieve_entry_stream(
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

    pub async fn retrieve_entry(
        conn: &impl GenericClient,
        entries_id: &EntryId,
    ) -> Result<Vec<Self>, PgError> {
        let stream = Self::retrieve_entry_stream(conn, entries_id).await?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(try_record) = stream.next().await {
            let record = try_record?;

            rtn.push(record);
        }

        Ok(rtn)
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

#[cfg(test)]
mod test {
    use super::*;

    use chrono::{Utc, Duration};

    const INT: Type = Type::Integer {
        minimum: Some(1),
        maximum: Some(10),
    };
    const INT_LOW: Type = Type::Integer {
        minimum: Some(1),
        maximum: None,
    };
    const INT_HIGH: Type = Type::Integer {
        minimum: None,
        maximum: Some(10),
    };
    const INT_NO_LIMIT: Type = Type::Integer {
        minimum: None,
        maximum: None,
    };

    const INT_RANGE: Type = Type::IntegerRange {
        minimum: Some(1),
        maximum: Some(10),
    };
    const INT_RANGE_LOW: Type = Type::IntegerRange {
        minimum: Some(1),
        maximum: None,
    };
    const INT_RANGE_HIGH: Type = Type::IntegerRange {
        minimum: None,
        maximum: Some(10),
    };
    const INT_RANGE_NO_LIMIT: Type = Type::IntegerRange {
        minimum: None,
        maximum: None,
    };

    const FLOAT: Type = Type::Float {
        minimum: Some(1.0),
        maximum: Some(10.0),
        step: 0.1,
        precision: 2,
    };
    const FLOAT_LOW: Type = Type::Float {
        minimum: Some(1.0),
        maximum: None,
        step: 0.1,
        precision: 2,
    };
    const FLOAT_HIGH: Type = Type::Float {
        minimum: None,
        maximum: Some(10.0),
        step: 0.1,
        precision: 2,
    };
    const FLOAT_NO_LIMIT: Type = Type::Float {
        minimum: None,
        maximum: None,
        step: 0.1,
        precision: 2,
    };

    const FLOAT_RANGE: Type = Type::FloatRange {
        minimum: Some(1.0),
        maximum: Some(10.0),
        step: 0.1,
        precision: 2,
    };
    const FLOAT_RANGE_LOW: Type = Type::FloatRange {
        minimum: Some(1.0),
        maximum: None,
        step: 0.1,
        precision: 2,
    };
    const FLOAT_RANGE_HIGH: Type = Type::FloatRange {
        minimum: None,
        maximum: Some(10.0),
        step: 0.1,
        precision: 2,
    };
    const FLOAT_RANGE_NO_LIMIT: Type = Type::FloatRange {
        minimum: None,
        maximum: None,
        step: 0.1,
        precision: 2,
    };

    const TIME: Type = Type::Time {
        as_12hr: false
    };
    const TIME_RANGE: Type = Type::TimeRange {
        show_diff: false,
        as_12hr: false
    };

    #[test]
    fn integer() {
        let given = Value::Integer { value: 5 };
        let given_low = Value::Integer { value: 1 };
        let given_high = Value::Integer { value: 10 };

        assert!(INT.validate(given).is_ok());
        assert!(INT.validate(given_low).is_ok());
        assert!(INT.validate(given_high).is_ok());
    }

    #[test]
    fn integer_low() {
        let given = Value::Integer { value: 5 };
        let given_low = Value::Integer { value: 1 };
        let given_high = Value::Integer { value: i32::MAX };

        assert!(INT_LOW.validate(given).is_ok());
        assert!(INT_LOW.validate(given_low).is_ok());
        assert!(INT_LOW.validate(given_high).is_ok());
    }

    #[test]
    fn integer_high() {
        let given = Value::Integer { value: 5 };
        let given_low = Value::Integer { value: i32::MIN };
        let given_high = Value::Integer { value: 10 };

        assert!(INT_HIGH.validate(given).is_ok());
        assert!(INT_HIGH.validate(given_low).is_ok());
        assert!(INT_HIGH.validate(given_high).is_ok());
    }

    #[test]
    fn integer_no_limit() {
        let given = Value::Integer { value: 5 };
        let given_low = Value::Integer { value: i32::MIN };
        let given_high = Value::Integer { value: i32::MAX };

        assert!(INT_NO_LIMIT.validate(given).is_ok());
        assert!(INT_NO_LIMIT.validate(given_low).is_ok());
        assert!(INT_NO_LIMIT.validate(given_high).is_ok());
    }

    #[test]
    fn integer_mismatch() {
        let given = Value::IntegerRange { low: 0, high: 1 };

        assert!(INT.validate(given).is_err());
    }

    #[test]
    fn integer_range() {
        let given = Value::IntegerRange { low: 3, high: 7 };
        let given_low = Value::IntegerRange { low: 1, high: 7 };
        let given_high = Value::IntegerRange { low: 3, high: 10 };
        let given_bounds = Value::IntegerRange { low: 1, high: 10 };

        assert!(INT_RANGE.validate(given).is_ok());
        assert!(INT_RANGE.validate(given_low).is_ok());
        assert!(INT_RANGE.validate(given_high).is_ok());
        assert!(INT_RANGE.validate(given_bounds).is_ok());
    }

    #[test]
    fn integer_range_low() {
        let given = Value::IntegerRange { low: 3, high: 7 };
        let given_low = Value::IntegerRange { low: 1, high: i32::MAX };
        let given_high = Value::IntegerRange { low: 3, high: i32::MAX };

        assert!(INT_RANGE_LOW.validate(given).is_ok());
        assert!(INT_RANGE_LOW.validate(given_low).is_ok());
        assert!(INT_RANGE_LOW.validate(given_high).is_ok());
    }

    #[test]
    fn integer_range_high() {
        let given = Value::IntegerRange { low: 3, high: 7 };
        let given_low = Value::IntegerRange { low: i32::MIN, high: 7 };
        let given_high = Value::IntegerRange { low: i32::MIN, high: 10 };

        assert!(INT_RANGE_HIGH.validate(given).is_ok());
        assert!(INT_RANGE_HIGH.validate(given_low).is_ok());
        assert!(INT_RANGE_HIGH.validate(given_high).is_ok());
    }

    #[test]
    fn integer_range_no_limit() {
        let given = Value::IntegerRange { low: 3, high: 7 };
        let given_bounds = Value::IntegerRange { low: i32::MIN, high: i32::MAX };

        assert!(INT_RANGE_NO_LIMIT.validate(given).is_ok());
        assert!(INT_RANGE_NO_LIMIT.validate(given_bounds).is_ok());
    }

    #[test]
    fn integer_range_mismatch() {
        let given = Value::Integer { value: 5 };

        assert!(INT_RANGE.validate(given).is_err());
    }

    #[test]
    fn float() {
        let given = Value::Float { value: 5.0 };
        let given_low = Value::Float { value: 1.0 };
        let given_high = Value::Float { value: 10.0 };

        assert!(FLOAT.validate(given).is_ok());
        assert!(FLOAT.validate(given_low).is_ok());
        assert!(FLOAT.validate(given_high).is_ok());
    }

    #[test]
    fn float_low() {
        let given = Value::Float { value: 5.0 };
        let given_low = Value::Float { value: 1.0 };
        let given_high = Value::Float { value: f32::MAX };

        assert!(FLOAT_LOW.validate(given).is_ok());
        assert!(FLOAT_LOW.validate(given_low).is_ok());
        assert!(FLOAT_LOW.validate(given_high).is_ok());
    }

    #[test]
    fn float_high() {
        let given = Value::Float { value: 5.0 };
        let given_low = Value::Float { value: f32::MIN };
        let given_high = Value::Float { value: 10.0 };

        assert!(FLOAT_HIGH.validate(given).is_ok());
        assert!(FLOAT_HIGH.validate(given_low).is_ok());
        assert!(FLOAT_HIGH.validate(given_high).is_ok());
    }

    #[test]
    fn float_no_limit() {
        let given = Value::Float { value: 5.0 };
        let given_low = Value::Float { value: f32::MIN };
        let given_high = Value::Float { value: f32::MAX };

        assert!(FLOAT_NO_LIMIT.validate(given).is_ok());
        assert!(FLOAT_NO_LIMIT.validate(given_low).is_ok());
        assert!(FLOAT_NO_LIMIT.validate(given_high).is_ok());
    }

    #[test]
    fn float_mismatch() {
        let given = Value::Integer { value: 5 };

        assert!(FLOAT.validate(given).is_err());
    }

    #[test]
    fn float_range() {
        let given = Value::FloatRange { low: 3.0, high: 7.0 };
        let given_low = Value::FloatRange { low: 1.0, high: 7.0 };
        let given_high = Value::FloatRange { low: 3.0, high: 10.0 };
        let given_bounds = Value::FloatRange { low: 1.0, high: 10.0 };

        assert!(FLOAT_RANGE.validate(given).is_ok());
        assert!(FLOAT_RANGE.validate(given_low).is_ok());
        assert!(FLOAT_RANGE.validate(given_high).is_ok());
        assert!(FLOAT_RANGE.validate(given_bounds).is_ok());
    }

    #[test]
    fn float_range_low() {
        let given = Value::FloatRange { low: 3.0, high: 7.0 };
        let given_low = Value::FloatRange { low: 1.0, high: f32::MAX };
        let given_high = Value::FloatRange { low: 3.0, high: f32::MAX };

        assert!(FLOAT_RANGE_LOW.validate(given).is_ok());
        assert!(FLOAT_RANGE_LOW.validate(given_low).is_ok());
        assert!(FLOAT_RANGE_LOW.validate(given_high).is_ok());
    }

    #[test]
    fn float_range_high() {
        let given = Value::FloatRange { low: 3.0, high: 7.0 };
        let given_low = Value::FloatRange { low: f32::MIN, high: 7.0 };
        let given_high = Value::FloatRange { low: f32::MIN, high: 10.0 };

        assert!(FLOAT_RANGE_HIGH.validate(given).is_ok());
        assert!(FLOAT_RANGE_HIGH.validate(given_low).is_ok());
        assert!(FLOAT_RANGE_HIGH.validate(given_high).is_ok());
    }

    #[test]
    fn float_range_no_limit() {
        let given = Value::FloatRange { low: 3.0, high: 7.0 };
        let given_bounds = Value::FloatRange { low: f32::MIN, high: f32::MAX };

        assert!(FLOAT_RANGE_NO_LIMIT.validate(given).is_ok());
        assert!(FLOAT_RANGE_NO_LIMIT.validate(given_bounds).is_ok());
    }

    #[test]
    fn float_range_mismatch() {
        let given = Value::Integer { value: 5 };

        assert!(FLOAT_RANGE.validate(given).is_err());
    }

    #[test]
    fn time() {
        let given = Value::Time { value: Utc::now() };

        assert!(TIME.validate(given).is_ok());
    }

    #[test]
    fn time_mismatch() {
        let given = Value::Integer { value: 5 };

        assert!(TIME.validate(given).is_err());
    }

    #[test]
    fn time_range() {
        let given = Value::TimeRange {
            low: Utc::now(),
            high: Utc::now() + Duration::new(10, 0).unwrap(),
        };

        assert!(TIME_RANGE.validate(given).is_ok());
    }

    #[test]
    fn time_range_mismatch() {
        let given = Value::Integer { value: 5 };

        assert!(TIME_RANGE.validate(given).is_err());
    }
}
