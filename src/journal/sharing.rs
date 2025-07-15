use std::str::FromStr;

use bytes::BytesMut;
use chrono::{DateTime, Utc};
use postgres_types as pg_types;
use serde::{Deserialize, Serialize};

use crate::db::ids::{JournalId, JournalShareId};
use crate::error::BoxDynError;

#[derive(Debug)]
pub struct JournalShare {
    pub id: JournalShareId,
    pub journals_id: JournalId,
    pub name: String,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

pub struct JournalShareAbility {
    pub journal_shares_id: JournalShareId,
    pub ability: Ability,
}

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ability {
    JournalUpdate,
    EntryCreate,
    EntryUpdate,
    EntryDelete,
}

#[derive(Debug, thiserror::Error)]
#[error("the provided string is not a valid ability")]
pub struct InvalidAbility;

impl Ability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::JournalUpdate => "journal_update",
            Self::EntryCreate => "entry_create",
            Self::EntryUpdate => "entry_update",
            Self::EntryDelete => "entry_delete",
        }
    }
}

impl FromStr for Ability {
    type Err = InvalidAbility;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "journal_update" => Ok(Self::JournalUpdate),
            "entry_create" => Ok(Self::EntryCreate),
            "entry_update" => Ok(Self::EntryUpdate),
            "entry_delete" => Ok(Self::EntryDelete),
            _ => Err(InvalidAbility),
        }
    }
}

impl<'a> pg_types::FromSql<'a> for Ability {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let v = <&str as pg_types::FromSql>::from_sql(ty, raw)?;

        Ok(Self::from_str(v)?)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&str as pg_types::FromSql>::accepts(ty)
    }
}

impl pg_types::ToSql for Ability {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
        self.as_str().to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&str as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}
