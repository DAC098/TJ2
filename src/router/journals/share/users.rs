use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{JournalId, JournalShareId, UserId};
use crate::journal::sharing::JournalShare;
use crate::journal::Journal;
use crate::net::body;
use crate::net::Error;
use crate::sec::authn::Initiator;
use crate::sec::authz::{self, Ability, Scope};
use crate::state;

#[derive(Debug, Deserialize)]
pub struct ShareId {
    journals_id: JournalId,
    share_id: JournalShareId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachedUser {
    id: UserId,
    username: String,
    added: DateTime<Utc>,
}

impl AttachedUser {
    pub async fn retrieve(
        conn: &impl db::GenericClient,
        journal_shares_id: &JournalShareId,
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [journal_shares_id];

        Ok(conn
            .query_raw(
                "\
            select users.id, \
                   users.username, \
                   journal_share_users.added \
            from journal_share_users \
                left join users on \
                    journal_share_users.users_id = users.id \
            where journal_share_users.journal_shares_id = $1",
                params,
            )
            .await?
            .map(|result| {
                result.map(|row| Self {
                    id: row.get(0),
                    username: row.get(1),
                    added: row.get(2),
                })
            }))
    }
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum SearchUsersError {
    JournalNotFound,
    ShareNotFound,
}

impl IntoResponse for SearchUsersError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::ShareNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn search_users(
    state: state::SharedState,
    initiator: Initiator,
    Path(ShareId { journals_id, share_id }): Path<ShareId>,
) -> Result<body::Json<Vec<AttachedUser>>, Error<SearchUsersError>> {
    let conn = state.db().get().await?;

    authz::assert_permission(
        &conn,
        initiator.user.id,
        Scope::Journals,
        Ability::Read,
    )
    .await?;

    let journal = Journal::retrieve(&conn, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(Error::Inner(SearchUsersError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(Error::from(authz::PermissionError::Denied));
    }

    let share = JournalShare::retrieve(&conn, (&journal.id, &share_id))
        .await?
        .ok_or(Error::Inner(SearchUsersError::ShareNotFound))?;

    let users = AttachedUser::retrieve(&conn, &share.id)
        .await?
        .try_collect::<Vec<AttachedUser>>()
        .await?;

    Ok(body::Json(users))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum RemoveUser {
    Single {
        users_id: UserId,
    }
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum RemoveUserError {
    JournalNotFound,
    ShareNotFound,
    UserNotFound {
        users_id: Vec<UserId>
    },
}

impl IntoResponse for RemoveUserError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::ShareNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::UserNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn remove_user(
    state: state::SharedState,
    initiator: Initiator,
    Path(ShareId { journals_id, share_id }): Path<ShareId>,
    body::Json(kind): body::Json<RemoveUser>
) -> Result<StatusCode, Error<RemoveUserError>> {
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
        .ok_or(Error::Inner(RemoveUserError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(Error::from(authz::PermissionError::Denied));
    }

    let share = JournalShare::retrieve(&transaction, (&journal.id, &share_id))
        .await?
        .ok_or(Error::Inner(RemoveUserError::ShareNotFound))?;

    match kind {
        RemoveUser::Single { users_id } => {
            let result = transaction
                .execute(
                    "delete from journal_share_users where journal_shares_id = $1 and users_id = $2",
                    &[&share.id, &users_id]
                )
                .await?;

            if result != 1 {
                return Err(Error::Inner(RemoveUserError::UserNotFound {
                    users_id: vec![users_id]
                }));
            }
        }
    }

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
