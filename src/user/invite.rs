use bytes::BytesMut;
use chrono::{DateTime, Utc};
use postgres_types as pg_types;
use serde::{Serialize, Deserialize};

use crate::db;
use crate::db::ids::{UserId, InviteToken};
use crate::error::BoxDynError;

#[derive(Debug, Serialize, Deserialize)]
#[repr(i16)]
pub enum InviteStatus {
    Pending = 0,
    Accepted = 1,
    Rejected = 2,
}

#[derive(Debug, thiserror::Error)]
#[error("the provided status value is invalid")]
pub struct InvalidInviteStatus;

impl InviteStatus {
    pub fn is_pending(&self) -> bool {
        match self {
            Self::Pending => true,
            _ => false,
        }
    }
}

impl From<&InviteStatus> for i16 {
    fn from(value: &InviteStatus) -> Self {
        match value {
            InviteStatus::Pending => 0,
            InviteStatus::Accepted => 1,
            InviteStatus::Rejected => 2,
        }
    }
}

impl TryFrom<i16> for InviteStatus {
    type Error = InvalidInviteStatus;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(InviteStatus::Pending),
            1 => Ok(InviteStatus::Accepted),
            2 => Ok(InviteStatus::Rejected),
            _ => Err(InvalidInviteStatus)
        }
    }
}

impl<'a> pg_types::FromSql<'a> for InviteStatus {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let v = <i16 as pg_types::FromSql>::from_sql(ty, raw)?;

        Self::try_from(v).map_err(Into::into)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i16 as pg_types::FromSql>::accepts(ty)
    }
}

impl pg_types::ToSql for InviteStatus {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        let v: i16 = self.into();

        v.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i16 as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

#[derive(Debug)]
pub struct Invite {
    pub token: InviteToken,
    pub name: String,
    pub issued_on: DateTime<Utc>,
    pub expires_on: Option<DateTime<Utc>>,
    pub status: InviteStatus,
    pub users_id: Option<UserId>,
}

#[derive(Debug, thiserror::Error)]
pub enum InviteError {
    #[error("the action cannot be completed as the invite is not pending")]
    NotPending,

    #[error("the specified user does not exist")]
    UserNotFound,

    #[error(transparent)]
    Db(#[from] db::PgError)
}

pub enum InviteQuery<'a> {
    Token(&'a InviteToken)
}

impl<'a> From<&'a InviteToken> for InviteQuery<'a> {
    fn from(token: &'a InviteToken) -> Self {
        Self::Token(token)
    }
}

impl Invite {
    pub async fn retrieve<'a, T>(conn: &impl db::GenericClient, given: T) -> Result<Option<Self>, db::PgError>
    where
        T: Into<InviteQuery<'a>>
    {
        let result = match given.into() {
            InviteQuery::Token(token) => {
                conn.query_opt(
                    "\
                    select user_invites.token, \
                           user_invites.name, \
                           user_invites.issued_on, \
                           user_invites.expires_on, \
                           user_invites.status, \
                           user_invites.users_id \
                    from user_invites \
                    where token = $1",
                    &[token]
                ).await?
            }
        };

        Ok(result.map(|v| Self {
            token: v.get(0),
            name: v.get(1),
            issued_on: v.get(2),
            expires_on: v.get(3),
            status: v.get(4),
            users_id: v.get(5),
        }))
    }

    pub fn is_expired(&self) -> bool {
        let Some(expires_on) = self.expires_on.as_ref() else {
            return false;
        };

        *expires_on >= Utc::now()
    }

    pub async fn mark_accepted(
        &mut self,
        conn: &impl db::GenericClient,
        users_id: &UserId
    ) -> Result<(), InviteError> {
        if !self.status.is_pending() {
            return Err(InviteError::NotPending);
        }

        let status = InviteStatus::Accepted;
        let result = conn.execute(
            "\
            update user_invites \
            set status = $2, \
                users_id = $3 \
            where token = $1",
            &[&self.token, &status, users_id]
        ).await;

        if let Err(err) = result {
            if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::ForeignKey(constraint) => if constraint == "user_invites_users_id_fkey" {
                        return Err(InviteError::UserNotFound);
                    },
                    _ => {}
                }
            }

            Err(err.into())
        } else {
            self.status = status;
            self.users_id = Some(*users_id);

            Ok(())
        }
    }
}
