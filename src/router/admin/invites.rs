use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{GroupId, InviteToken, RoleId, UserId};
use crate::net::body;
use crate::net::Error;
use crate::sec::authn::Initiator;
use crate::sec::authz::{self, Role};
use crate::state;
use crate::user::group::Group;
use crate::user::invite::InviteStatus;

#[derive(Debug, Serialize)]
pub struct InviteFull {
    token: InviteToken,
    issued_on: DateTime<Utc>,
    expires_on: Option<DateTime<Utc>>,
    status: InviteStatus,
    user: Option<InviteUser>,
    role: Option<InviteRole>,
    group: Option<InviteGroup>,
}

#[derive(Debug, Serialize)]
pub struct InviteUser {
    id: UserId,
    username: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InviteRole {
    id: RoleId,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InviteGroup {
    id: GroupId,
    name: String,
}

pub async fn search_invites(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
) -> Result<body::Json<Vec<InviteFull>>, Error> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db_conn().await?;

    authz::assert_permission(
        &conn,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Read,
    )
    .await?;

    let params: db::ParamsArray<'_, 0> = [];
    let rtn = conn
        .query_raw(
            "\
        select user_invites.token, \
               user_invites.issued_on, \
               user_invites.expires_on, \
               user_invites.status, \
               users.id, \
               users.username, \
               authz_roles.id, \
               authz_roles.name, \
               groups.id, \
               groups.name \
        from user_invites
            left join users on \
                user_invites.users_id = users.id \
            left join authz_roles on \
                user_invites.role_id = authz_roles.id \
            left join groups on \
                user_invites.groups_id = groups.id \
        order by user_invites.status = 1 desc, \
                 user_invites.status = 2 desc, \
                 user_invites.status = 0 desc, \
                 user_invites.issued_on, \
                 users.username, \
                 user_invites.token",
            params,
        )
        .await?
        .map(|maybe| {
            maybe.map(|row| {
                let user = if let Some(id) = row.get::<usize, Option<UserId>>(4) {
                    Some(InviteUser {
                        id,
                        username: row.get(5),
                    })
                } else {
                    None
                };

                let role = if let Some(id) = row.get(6) {
                    Some(InviteRole {
                        id,
                        name: row.get(7),
                    })
                } else {
                    None
                };

                let group = if let Some(id) = row.get(8) {
                    Some(InviteGroup {
                        id,
                        name: row.get(9),
                    })
                } else {
                    None
                };

                InviteFull {
                    token: row.get(0),
                    issued_on: row.get(1),
                    expires_on: row.get(2),
                    status: row.get(3),
                    user,
                    role,
                    group,
                }
            })
        })
        .try_collect::<Vec<InviteFull>>()
        .await?;

    Ok(body::Json(rtn))
}

#[derive(Debug, Deserialize)]
pub struct NewInvite {
    amount: u32,
    expires_on: Option<DateTime<Utc>>,
    role_id: Option<RoleId>,
    groups_id: Option<GroupId>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum CreateInviteError {
    InvalidAmount,
    InvalidExpiresOn,
    RoleNotFound,
    GroupNotFound,
}

impl IntoResponse for CreateInviteError {
    fn into_response(self) -> Response {
        match self {
            Self::InvalidAmount => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::InvalidExpiresOn => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::RoleNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::GroupNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn create_invite(
    state: state::SharedState,
    initiator: Initiator,
    body::Json(NewInvite {
        amount,
        expires_on,
        role_id,
        groups_id,
    }): body::Json<NewInvite>,
) -> Result<body::Json<Vec<InviteFull>>, Error<CreateInviteError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Create,
    )
    .await?;

    if amount == 0 || amount > 10 {
        return Err(Error::Inner(CreateInviteError::InvalidAmount));
    }

    let now = Utc::now();

    if let Some(given) = expires_on.as_ref() {
        if *given <= now {
            return Err(Error::Inner(CreateInviteError::InvalidExpiresOn));
        }
    }

    let role = if let Some(role_id) = role_id {
        Some(
            Role::retrieve(&transaction, &role_id)
                .await?
                .map(|role| InviteRole {
                    id: role.id,
                    name: role.name,
                })
                .ok_or(Error::Inner(CreateInviteError::RoleNotFound))?,
        )
    } else {
        None
    };

    let group = if let Some(groups_id) = groups_id {
        Some(
            Group::retrieve(&transaction, &groups_id)
                .await?
                .map(|group| InviteGroup {
                    id: group.id,
                    name: group.name,
                })
                .ok_or(Error::Inner(CreateInviteError::GroupNotFound))?,
        )
    } else {
        None
    };

    let issued_on = Utc::now();
    let status = InviteStatus::Pending;
    let mut invites = Vec::with_capacity(amount as usize);

    for _ in 0..amount {
        invites.push(InviteFull {
            token: InviteToken::gen(),
            issued_on,
            expires_on,
            status,
            user: None,
            role: role.clone(),
            group: group.clone(),
        });
    }

    {
        let mut params: db::ParamsVec<'_> =
            vec![&issued_on, &expires_on, &status, &role_id, &groups_id];
        let mut query = String::from(
            "insert into user_invites (token, issued_on, expires_on, status, role_id, groups_id) values "
        );

        for (index, record) in invites.iter().enumerate() {
            if index > 0 {
                query.push_str(", ");
            }

            let segment = format!(
                "(${}, $1, $2, $3, $4, $5)",
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
    Single { token: InviteToken },
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum DeleteError {
    NotFound { tokens: Vec<InviteToken> },
}

impl IntoResponse for DeleteError {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn delete_invite(
    state: state::SharedState,
    initiator: Initiator,
    body::Json(kind): body::Json<DeleteInvite>,
) -> Result<StatusCode, Error<DeleteError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Create,
    )
    .await?;

    match kind {
        DeleteInvite::Single { token } => {
            let result = transaction
                .execute("delete from user_invites where token = $1", &[&token])
                .await?;

            if result != 1 {
                return Err(Error::Inner(DeleteError::NotFound {
                    tokens: vec![token],
                }));
            }
        }
    }

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
