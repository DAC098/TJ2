use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{JournalId, JournalShareId, JournalShareInviteToken, UserId};
use crate::journal::sharing::{JournalShare, JournalShareInviteStatus};
use crate::journal::Journal;
use crate::net::body;
use crate::net::Error;
use crate::sec::authn::Initiator;
use crate::sec::authz::{self, Ability, Scope};
use crate::state;

#[derive(Debug, Deserialize)]
pub struct SharePath {
    journals_id: JournalId,
    share_id: JournalShareId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InviteFull {
    token: JournalShareInviteToken,
    user: Option<InviteUser>,
    issued_on: DateTime<Utc>,
    expires_on: Option<DateTime<Utc>>,
    status: JournalShareInviteStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InviteUser {
    id: UserId,
    username: String,
}

impl InviteFull {
    pub async fn retrieve(
        conn: &impl db::GenericClient,
        journal_shares_id: &JournalShareId,
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [journal_shares_id];

        Ok(conn
            .query_raw(
                "\
            select journal_share_invites.token, \
                   users.id, \
                   users.username, \
                   journal_share_invites.issued_on, \
                   journal_share_invites.expires_on, \
                   journal_share_invites.status \
            from journal_share_invites \
                left join users on \
                    journal_share_invites.users_id = users.id \
            where journal_share_invites.journal_shares_id = $1 \
            order by journal_share_invites.status = 1 desc, \
                     journal_share_invites.status = 2 desc, \
                     journal_share_invites.status = 0 desc, \
                     journal_share_invites.issued_on",
                params,
            )
            .await?
            .map(|result| {
                result.map(|row| {
                    let user = if let Some(id) = row.get::<usize, Option<UserId>>(1) {
                        Some(InviteUser {
                            id,
                            username: row.get(2),
                        })
                    } else {
                        None
                    };

                    Self {
                        token: row.get(0),
                        user,
                        issued_on: row.get(3),
                        expires_on: row.get(4),
                        status: row.get(5),
                    }
                })
            }))
    }
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum SearchInvitesError {
    JournalNotFound,
    ShareNotFound,
}

impl IntoResponse for SearchInvitesError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::ShareNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn search_invites(
    state: state::SharedState,
    initiator: Initiator,
    Path(SharePath {
        journals_id,
        share_id,
    }): Path<SharePath>,
) -> Result<body::Json<Vec<InviteFull>>, Error<SearchInvitesError>> {
    let conn = state.db().get().await?;

    authz::assert_permission(&conn, initiator.user.id, Scope::Journals, Ability::Read).await?;

    let journal = Journal::retrieve(&conn, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(Error::Inner(SearchInvitesError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(Error::from(authz::PermissionError::Denied));
    }

    let share = JournalShare::retrieve(&conn, (&journal.id, &share_id))
        .await?
        .ok_or(Error::Inner(SearchInvitesError::ShareNotFound))?;

    let invites = InviteFull::retrieve(&conn, &share.id)
        .await?
        .try_collect::<Vec<InviteFull>>()
        .await?;

    Ok(body::Json(invites))
}

#[derive(Debug, Deserialize)]
pub struct NewInvite {
    amount: u32,
    expires_on: Option<DateTime<Utc>>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum CreateInviteError {
    JournalNotFound,
    ShareNotFound,
    InvalidAmount,
    InvalidExpiresOn,
}

impl IntoResponse for CreateInviteError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::ShareNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::InvalidAmount => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::InvalidExpiresOn => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
        }
    }
}

pub async fn create_invite(
    state: state::SharedState,
    initiator: Initiator,
    Path(SharePath {
        journals_id,
        share_id,
    }): Path<SharePath>,
    body::Json(NewInvite { amount, expires_on }): body::Json<NewInvite>,
) -> Result<body::Json<Vec<InviteFull>>, Error<CreateInviteError>> {
    if amount == 0 || amount > 10 {
        return Err(Error::Inner(CreateInviteError::InvalidAmount));
    }

    let now = Utc::now();

    if let Some(expires) = &expires_on {
        if *expires <= now {
            return Err(Error::Inner(CreateInviteError::InvalidExpiresOn));
        }
    }

    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        Scope::Journals,
        Ability::Update,
    )
    .await?;

    let journal = Journal::retrieve(&transaction, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(Error::Inner(CreateInviteError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(Error::from(authz::PermissionError::Denied));
    }

    let share = JournalShare::retrieve(&transaction, (&journal.id, &share_id))
        .await?
        .ok_or(Error::Inner(CreateInviteError::ShareNotFound))?;

    let issued_on = Utc::now();
    let status = JournalShareInviteStatus::Pending;
    let mut invites = Vec::with_capacity(amount as usize);

    for _ in 0..amount {
        invites.push(InviteFull {
            token: JournalShareInviteToken::gen(),
            issued_on,
            expires_on,
            status,
            user: None,
        });
    }

    {
        let mut params: db::ParamsVec<'_> = vec![&share.id, &issued_on, &status];
        let mut query = String::from(
            "insert into journal_share_invites (token, journal_shares_id, issued_on, status) values "
        );

        for (index, record) in invites.iter().enumerate() {
            if index > 0 {
                query.push_str(", ");
            }

            let segment = format!(
                "(${}, $1, $2, $3)",
                db::push_param(&mut params, &record.token)
            );

            query.push_str(&segment);
        }

        transaction.execute(&query, params.as_slice()).await?;
    }

    transaction.commit().await?;

    Ok(body::Json(invites))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum DeleteInvite {
    Single { token: JournalShareInviteToken },
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum DeleteInviteError {
    JournalNotFound,
    ShareNotFound,
    TokensNotFound {
        tokens: Vec<JournalShareInviteToken>,
    },
}

impl IntoResponse for DeleteInviteError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::ShareNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::TokensNotFound { .. } => {
                (StatusCode::NOT_FOUND, body::Json(self)).into_response()
            }
        }
    }
}

pub async fn delete_invite(
    state: state::SharedState,
    initiator: Initiator,
    Path(SharePath {
        journals_id,
        share_id,
    }): Path<SharePath>,
    body::Json(kind): body::Json<DeleteInvite>,
) -> Result<StatusCode, Error<DeleteInviteError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        Scope::Journals,
        Ability::Update,
    )
    .await?;

    let journal = Journal::retrieve(&transaction, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(Error::Inner(DeleteInviteError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(Error::from(authz::PermissionError::Denied));
    }

    let _share = JournalShare::retrieve(&transaction, (&journal.id, &share_id))
        .await?
        .ok_or(Error::Inner(DeleteInviteError::ShareNotFound))?;

    match kind {
        DeleteInvite::Single { token } => {
            let result = transaction
                .execute(
                    "delete from journal_share_invites where token = $1",
                    &[&token],
                )
                .await?;

            if result != 1 {
                return Err(Error::Inner(DeleteInviteError::TokensNotFound {
                    tokens: vec![token],
                }));
            }
        }
    }

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
