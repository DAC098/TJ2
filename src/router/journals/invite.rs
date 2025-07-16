use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::db::ids::{JournalId, JournalShareId, JournalShareInviteToken};
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

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum CreateInviteError {
    JournalNotFound,
    ShareNotFound,
}

impl IntoResponse for CreateInviteError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::ShareNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn create_invite(
    state: state::SharedState,
    initiator: Initiator,
    Path(SharePath { journals_id, share_id }): Path<SharePath>,
) -> Result<body::Json<Vec<JournalShareInviteToken>>, Error<CreateInviteError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(&transaction, initiator.user.id, Scope::Journals, Ability::Update).await?;

    let journal = Journal::retrieve(&transaction, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(Error::Inner(CreateInviteError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(Error::from(authz::PermissionError::Denied));
    }

    let share = JournalShare::retrieve(&transaction, (&journal.id, &share_id))
        .await?
        .ok_or(Error::Inner(CreateInviteError::ShareNotFound))?;

    let token = JournalShareInviteToken::gen();
    let issued_on = Utc::now();
    let status = JournalShareInviteStatus::Pending;

    transaction.execute(
        "\
        insert into journal_share_invites (token, journal_shares_id, issued_on, status) values \
        ($1, $2, $3, $4)",
        &[&token, &share.id, &issued_on, &status]
    ).await?;

    transaction.commit().await?;

    Ok(body::Json(vec![token]))
}

#[derive(Debug, Deserialize)]
pub enum DeleteInvite {
    Token(JournalShareInviteToken),
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum DeleteInviteError {
    JournalNotFound,
    ShareNotFound,
    TokensNotFound {
        tokens: Vec<JournalShareInviteToken>,
    }
}

impl IntoResponse for DeleteInviteError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::ShareNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::TokensNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn delete_invite(
    state: state::SharedState,
    initiator: Initiator,
    Path(SharePath { journals_id, share_id }): Path<SharePath>,
    body::Json(kind): body::Json<DeleteInvite>,
) -> Result<StatusCode, Error<DeleteInviteError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(&transaction, initiator.user.id, Scope::Journals, Ability::Update).await?;

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
        DeleteInvite::Token(token) => {
            let result = transaction.execute(
                "delete from journal_share_invites where token = $1",
                &[&token]
            ).await?;

            if result != 1 {
                return Err(Error::Inner(DeleteInviteError::TokensNotFound {
                    tokens: vec![token]
                }));
            }
        }
    }

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
