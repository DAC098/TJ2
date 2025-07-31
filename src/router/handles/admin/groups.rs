use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{GroupId, GroupUid, RoleId, UserId};
use crate::net::{body, Error};
use crate::sec::authn::Initiator;
use crate::sec::authz::{
    self, create_attached_roles, update_attached_roles, AttachedRole, AttachedRoleError,
};
use crate::state;
use crate::user::group::Group;
use crate::user::{create_attached_users, update_attached_users, AttachedUser, AttachedUserError};

#[derive(Debug, Serialize)]
pub struct GroupPartial {
    id: GroupId,
    uid: GroupUid,
    name: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
}

pub async fn retrieve_groups(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
) -> Result<body::Json<Vec<GroupPartial>>, Error> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db_conn().await?;

    authz::assert_permission(
        &conn,
        initiator.user.id,
        authz::Scope::Groups,
        authz::Ability::Read,
    )
    .await?;

    let params: db::ParamsArray<'_, 0> = [];
    let groups = conn
        .query_raw(
            "\
        with search_groups as ( \
            select * \
            from groups \
        ) \
        select search_groups.id, \
               search_groups.uid, \
               search_groups.name, \
               search_groups.created, \
               search_groups.updated \
        from search_groups \
        order by search_groups.name",
            params,
        )
        .await?;

    futures::pin_mut!(groups);

    let mut rtn = Vec::new();

    while let Some(result) = groups.next().await {
        let record = result?;

        rtn.push(GroupPartial {
            id: record.get(0),
            uid: record.get(1),
            name: record.get(2),
            created: record.get(3),
            updated: record.get(4),
        });
    }

    Ok(body::Json(rtn))
}

#[derive(Debug, Deserialize)]
pub struct GroupPath {
    groups_id: GroupId,
}

#[derive(Debug, Serialize)]
pub struct GroupFull {
    id: GroupId,
    uid: GroupUid,
    name: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
    users: Vec<AttachedUser>,
    roles: Vec<AttachedRole>,
}

impl GroupFull {
    async fn retrieve(
        conn: &impl db::GenericClient,
        groups_id: &GroupId,
    ) -> Result<Option<Self>, db::PgError> {
        let result = Group::retrieve_id(conn, *groups_id).await?;

        let Some(group) = result else {
            return Ok(None);
        };

        let (users, roles) = tokio::join!(
            AttachedUser::retrieve(conn, &group),
            AttachedRole::retrieve(conn, &group)
        );

        Ok(Some(Self {
            id: group.id,
            uid: group.uid,
            name: group.name,
            created: group.created,
            updated: group.updated,
            users: users?,
            roles: roles?,
        }))
    }
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum RetrieveGroupError {
    GroupNotFound,
}

impl IntoResponse for RetrieveGroupError {
    fn into_response(self) -> Response {
        match self {
            Self::GroupNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn retrieve_group(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(GroupPath { groups_id }): Path<GroupPath>,
) -> Result<body::Json<GroupFull>, Error<RetrieveGroupError>> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db_conn().await?;

    authz::assert_permission(
        &conn,
        initiator.user.id,
        authz::Scope::Groups,
        authz::Ability::Read,
    )
    .await?;

    let group = GroupFull::retrieve(&conn, &groups_id)
        .await?
        .ok_or(Error::Inner(RetrieveGroupError::GroupNotFound))?;

    Ok(body::Json(group))
}

#[derive(Debug, Deserialize)]
pub struct NewGroup {
    name: String,
    users: Vec<UserId>,
    roles: Vec<RoleId>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum NewGroupError {
    GroupExists,
    UsersNotFound { ids: Vec<UserId> },
    RolesNotFound { ids: Vec<RoleId> },
}

impl IntoResponse for NewGroupError {
    fn into_response(self) -> Response {
        match self {
            Self::GroupExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::UsersNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::RolesNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn create_group(
    db::Conn(mut conn): db::Conn,
    initiator: Initiator,
    body::Json(json): body::Json<NewGroup>,
) -> Result<body::Json<GroupFull>, Error<NewGroupError>> {
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Groups,
        authz::Ability::Create,
    )
    .await?;

    let group = Group::create(&transaction, &json.name)
        .await?
        .ok_or(Error::Inner(NewGroupError::GroupExists))?;

    let users = match create_attached_users(&transaction, &group, json.users).await {
        Ok(users) => users,
        Err(err) => {
            return Err(match err {
                AttachedUserError::NotFound(ids) => {
                    Error::Inner(NewGroupError::UsersNotFound { ids })
                }
                AttachedUserError::Db(err) => Error::from(err),
            })
        }
    };

    let roles = match create_attached_roles(&transaction, &group, json.roles).await {
        Ok(roles) => roles,
        Err(err) => {
            return Err(match err {
                AttachedRoleError::NotFound(ids) => {
                    Error::Inner(NewGroupError::RolesNotFound { ids })
                }
                AttachedRoleError::Db(err) => Error::from(err),
            })
        }
    };

    transaction.commit().await?;

    Ok(body::Json(GroupFull {
        id: group.id,
        uid: group.uid,
        name: group.name,
        created: group.created,
        updated: group.updated,
        users,
        roles,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroup {
    name: Option<String>,
    users: Option<Vec<UserId>>,
    roles: Option<Vec<RoleId>>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum UpdateGroupError {
    GroupExists,
    GroupNotFound,
    UsersNotFound { ids: Vec<UserId> },
    RolesNotFound { ids: Vec<RoleId> },
}

impl IntoResponse for UpdateGroupError {
    fn into_response(self) -> Response {
        match self {
            Self::GroupExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::GroupNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::UsersNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::RolesNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn update_group(
    db::Conn(mut conn): db::Conn,
    initiator: Initiator,
    Path(GroupPath { groups_id }): Path<GroupPath>,
    body::Json(json): body::Json<UpdateGroup>,
) -> Result<StatusCode, Error<UpdateGroupError>> {
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Groups,
        authz::Ability::Update,
    )
    .await?;

    let mut group = Group::retrieve_id(&transaction, groups_id)
        .await?
        .ok_or(Error::Inner(UpdateGroupError::GroupNotFound))?;

    if json.name.is_some() {
        if let Some(name) = json.name {
            group.name = name;
        }

        let did_update = group.update(&transaction).await?;

        if !did_update {
            return Err(Error::Inner(UpdateGroupError::GroupExists));
        }
    }

    match update_attached_users(&transaction, &group, json.users).await {
        Ok(_) => {}
        Err(err) => {
            return Err(match err {
                AttachedUserError::NotFound(ids) => {
                    Error::Inner(UpdateGroupError::UsersNotFound { ids })
                }
                AttachedUserError::Db(err) => Error::from(err),
            })
        }
    };

    match update_attached_roles(&transaction, &group, json.roles).await {
        Ok(_) => {}
        Err(err) => {
            return Err(match err {
                AttachedRoleError::NotFound(ids) => {
                    Error::Inner(UpdateGroupError::RolesNotFound { ids })
                }
                AttachedRoleError::Db(err) => Error::from(err),
            })
        }
    }

    transaction.commit().await?;

    Ok(StatusCode::OK)
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum DeleteGroupError {
    GroupNotFound,
}

impl IntoResponse for DeleteGroupError {
    fn into_response(self) -> Response {
        match self {
            Self::GroupNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn delete_group(
    db::Conn(mut conn): db::Conn,
    initiator: Initiator,
    Path(GroupPath { groups_id }): Path<GroupPath>,
) -> Result<StatusCode, Error<DeleteGroupError>> {
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Groups,
        authz::Ability::Delete,
    )
    .await?;

    let group = GroupFull::retrieve(&transaction, &groups_id)
        .await?
        .ok_or(Error::Inner(DeleteGroupError::GroupNotFound))?;

    let _users = transaction
        .execute("delete from group_users where groups_id = $1", &[&group.id])
        .await?;

    let _roles = transaction
        .execute("delete from group_roles where groups_id = $1", &[&group.id])
        .await?;

    let _group = transaction
        .execute("delete from groups where id = $1", &[&group.id])
        .await?;

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
