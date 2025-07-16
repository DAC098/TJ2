use std::str::FromStr;

use bytes::BytesMut;
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use postgres_types as pg_types;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{UserId, JournalId, JournalShareId, JournalShareInviteToken};
use crate::error::BoxDynError;

#[derive(Debug)]
pub struct JournalShare {
    pub id: JournalShareId,
    pub journals_id: JournalId,
    pub name: String,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub enum RetrieveShare<'a> {
    JournalAndId((&'a JournalId, &'a JournalShareId)),
}

impl<'a> From<(&'a JournalId, &'a JournalShareId)> for RetrieveShare<'a> {
    fn from(given: (&'a JournalId, &'a JournalShareId)) -> Self {
        Self::JournalAndId(given)
    }
}

impl JournalShare {
    pub async fn retrieve<'a, T>(
        conn: &impl db::GenericClient,
        given: T,
    ) -> Result<Option<Self>, db::PgError>
    where
        T: Into<RetrieveShare<'a>>,
    {
        let result = match given.into() {
            RetrieveShare::JournalAndId((journals_id, journal_shares_id)) => {
                conn.query_opt(
                    "\
                select journal_shares.id, \
                       journal_shares.journals_id, \
                       journal_shares.name, \
                       journal_shares.created, \
                       journal_shares.updated \
                from journal_shares \
                where journal_shares.id = $1 and \
                      journal_shares.journals_id = $2",
                    &[journal_shares_id, journals_id],
                )
                .await?
            }
        };

        Ok(result.map(|row| Self {
            id: row.get(0),
            journals_id: row.get(1),
            name: row.get(2),
            created: row.get(3),
            updated: row.get(4),
        }))
    }
}

#[derive(Debug)]
pub struct JournalShareInvite {
    pub token: JournalShareInviteToken,
    pub journal_shares_id: JournalShareId,
    pub users_id: Option<UserId>,
    pub issued_on: DateTime<Utc>,
    pub expires_on: Option<DateTime<Utc>>,
    pub status: JournalShareInviteStatus,
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(i16)]
pub enum JournalShareInviteStatus {
    Pending = 0,
    Accepted = 1,
    Rejected = 2,
}

#[derive(Debug, thiserror::Error)]
#[error("the provided status value is invalid")]
pub struct InvalidJournalShareInviteStatus;

impl JournalShareInviteStatus {
    pub fn is_pending(&self) -> bool {
        match self {
            Self::Pending => true,
            _ => false,
        }
    }
}

impl From<&JournalShareInviteStatus> for i16 {
    fn from(value: &JournalShareInviteStatus) -> Self {
        match value {
            JournalShareInviteStatus::Pending => 0,
            JournalShareInviteStatus::Accepted => 1,
            JournalShareInviteStatus::Rejected => 2,
        }
    }
}

impl TryFrom<i16> for JournalShareInviteStatus {
    type Error = InvalidJournalShareInviteStatus;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(JournalShareInviteStatus::Pending),
            1 => Ok(JournalShareInviteStatus::Accepted),
            2 => Ok(JournalShareInviteStatus::Rejected),
            _ => Err(InvalidJournalShareInviteStatus),
        }
    }
}

impl<'a> pg_types::FromSql<'a> for JournalShareInviteStatus {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let v = <i16 as pg_types::FromSql>::from_sql(ty, raw)?;

        Self::try_from(v).map_err(Into::into)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i16 as pg_types::FromSql>::accepts(ty)
    }
}

impl pg_types::ToSql for JournalShareInviteStatus {
    fn to_sql(
        &self,
        ty: &pg_types::Type,
        w: &mut BytesMut,
    ) -> Result<pg_types::IsNull, BoxDynError> {
        let v: i16 = self.into();

        v.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i16 as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
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
    pub async fn retrieve(conn: &impl db::GenericClient, journal_shares_id: &JournalShareId) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [journal_shares_id];

        Ok(conn.query_raw(
            "\
            select journal_share_abilities.ability \
            from journal_share_abilities \
            where journal_share_abilities.journal_shares_id = $1",
            params
        ).await?.map(|result| result.map(|record| record.get(0))))
    }

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
