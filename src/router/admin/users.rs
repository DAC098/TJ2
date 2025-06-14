use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{GroupId, RoleId, UserId, UserUid};
use crate::net::body;
use crate::net::Error;
use crate::sec::authn::Initiator;
use crate::sec::authz::{
    create_attached_roles, update_attached_roles, AttachedRole, AttachedRoleError,
};
use crate::sec::{authz, password};
use crate::state;
use crate::user::group::{
    create_attached_groups, update_attached_groups, AttachedGroup, AttachedGroupError,
};
use crate::user::{User, UserBuilder, UserBuilderError};

#[derive(Debug, Serialize)]
pub struct UserPartial {
    id: UserId,
    uid: UserUid,
    username: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
}

pub async fn search_users(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
) -> Result<body::Json<Vec<UserPartial>>, Error> {
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
    let users = conn
        .query_raw(
            "\
        with search_users as ( \
            select * \
            from users \
        ) \
        select search_users.id, \
               search_users.uid, \
               search_users.username, \
               search_users.created, \
               search_users.updated \
        from search_users \
        order by search_users.username",
            params,
        )
        .await?;

    futures::pin_mut!(users);

    let mut found = Vec::new();

    while let Some(result) = users.next().await {
        let record = result?;

        found.push(UserPartial {
            id: record.get(0),
            uid: record.get(1),
            username: record.get(2),
            created: record.get(3),
            updated: record.get(4),
        });
    }

    Ok(body::Json(found))
}

#[derive(Debug, Deserialize)]
pub struct UserPath {
    users_id: UserId,
}

#[derive(Debug, Serialize)]
pub struct UserFull {
    id: UserId,
    uid: UserUid,
    username: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
    groups: Vec<AttachedGroup>,
    roles: Vec<AttachedRole>,
}

impl UserFull {
    async fn retrieve(
        conn: &impl db::GenericClient,
        users_id: &UserId,
    ) -> Result<Option<Self>, db::PgError> {
        let result = User::retrieve_id(conn, *users_id).await?;

        let Some(user) = result else {
            return Ok(None);
        };

        let (groups, roles) = tokio::join!(
            AttachedGroup::retrieve(conn, &user),
            AttachedRole::retrieve(conn, &user),
        );

        Ok(Some(Self {
            id: user.id,
            uid: user.uid,
            username: user.username,
            created: user.created,
            updated: user.updated,
            groups: groups?,
            roles: roles?,
        }))
    }
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum RetrieveUserError {
    UserNotFound,
}

impl IntoResponse for RetrieveUserError {
    fn into_response(self) -> Response {
        match self {
            Self::UserNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn retrieve_user(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(UserPath { users_id }): Path<UserPath>,
) -> Result<body::Json<UserFull>, Error<RetrieveUserError>> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db().get().await?;

    authz::assert_permission(
        &conn,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Read,
    )
    .await?;

    let user = UserFull::retrieve(&conn, &users_id)
        .await?
        .ok_or(Error::Inner(RetrieveUserError::UserNotFound))?;

    Ok(body::Json(user))
}

#[derive(Debug, Deserialize)]
pub struct NewUser {
    username: String,
    password: String,
    groups: Vec<GroupId>,
    roles: Vec<RoleId>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum CreateUserError {
    UsernameExists,
    GroupsNotFound { ids: Vec<GroupId> },
    RolesNotFound { ids: Vec<RoleId> },
}

impl IntoResponse for CreateUserError {
    fn into_response(self) -> Response {
        match self {
            Self::UsernameExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::GroupsNotFound { .. } => {
                (StatusCode::NOT_FOUND, body::Json(self)).into_response()
            }
            Self::RolesNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn create_user(
    db::Conn(mut conn): db::Conn,
    storage: state::Storage,
    initiator: Initiator,
    body::Json(json): body::Json<NewUser>,
) -> Result<body::Json<UserFull>, Error<CreateUserError>> {
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Create,
    )
    .await?;

    let builder = UserBuilder::new_password(json.username, json.password);
    let user = match builder.build(&transaction).await {
        Ok(user) => user,
        Err(err) => {
            return Err(match err {
                UserBuilderError::UsernameExists => Error::Inner(CreateUserError::UsernameExists),
                UserBuilderError::UidExists => Error::message("user uid collision"),
                UserBuilderError::Db(err) => Error::from(err),
                UserBuilderError::Argon(err) => Error::from(err),
            })
        }
    };

    let groups = match create_attached_groups(&transaction, &user, json.groups).await {
        Ok(groups) => groups,
        Err(err) => {
            return Err(match err {
                AttachedGroupError::NotFound(ids) => {
                    Error::Inner(CreateUserError::GroupsNotFound { ids })
                }
                AttachedGroupError::Db(err) => Error::from(err),
            })
        }
    };

    let roles = match create_attached_roles(&transaction, &user, json.roles).await {
        Ok(roles) => roles,
        Err(err) => {
            return Err(match err {
                AttachedRoleError::NotFound(ids) => {
                    Error::Inner(CreateUserError::RolesNotFound { ids })
                }
                AttachedRoleError::Db(err) => Error::from(err),
            })
        }
    };

    let user_dir = storage.user_dir(user.id);

    user_dir.create().await?;

    let private_key = tj2_lib::sec::pki::PrivateKey::generate()?;

    private_key.save(user_dir.private_key(), false).await?;

    transaction.commit().await?;

    Ok(body::Json(UserFull {
        id: user.id,
        uid: user.uid,
        username: user.username,
        created: user.created,
        updated: user.updated,
        groups,
        roles,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UpdateUser {
    username: Option<String>,
    password: Option<String>,
    groups: Option<Vec<GroupId>>,
    roles: Option<Vec<RoleId>>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum UpdatedUserError {
    UserNotFound,
    UsernameExists,
    GroupsNotFound { ids: Vec<GroupId> },
    RolesNotFound { ids: Vec<RoleId> },
}

impl IntoResponse for UpdatedUserError {
    fn into_response(self) -> Response {
        match self {
            Self::UserNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::UsernameExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::GroupsNotFound { .. } => {
                (StatusCode::NOT_FOUND, body::Json(self)).into_response()
            }
            Self::RolesNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn update_user(
    db::Conn(mut conn): db::Conn,
    initiator: Initiator,
    Path(UserPath { users_id }): Path<UserPath>,
    body::Json(json): body::Json<UpdateUser>,
) -> Result<StatusCode, Error<UpdatedUserError>> {
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Update,
    )
    .await?;

    let mut user = User::retrieve_id(&transaction, users_id)
        .await?
        .ok_or(Error::Inner(UpdatedUserError::UserNotFound))?;

    if json.username.is_some() || json.password.is_some() {
        if let Some(username) = json.username {
            user.username = username;
        }

        if let Some(password) = &json.password {
            user.password = password::create(password)?;
            user.version = 0;
        }

        if !user.update(&transaction).await? {
            return Err(Error::Inner(UpdatedUserError::UsernameExists));
        }
    }

    match update_attached_groups(&transaction, &user, json.groups).await {
        Ok(_) => {}
        Err(err) => {
            return Err(match err {
                AttachedGroupError::NotFound(ids) => {
                    Error::Inner(UpdatedUserError::GroupsNotFound { ids })
                }
                AttachedGroupError::Db(err) => Error::from(err),
            })
        }
    };

    match update_attached_roles(&transaction, &user, json.roles).await {
        Ok(_) => {}
        Err(err) => {
            return Err(match err {
                AttachedRoleError::NotFound(ids) => {
                    Error::Inner(UpdatedUserError::RolesNotFound { ids })
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
pub enum DeleteUserError {
    UserNotFound,
}

impl IntoResponse for DeleteUserError {
    fn into_response(self) -> Response {
        match self {
            Self::UserNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn delete_user(
    db::Conn(mut conn): db::Conn,
    initiator: Initiator,
    Path(UserPath { users_id }): Path<UserPath>,
) -> Result<StatusCode, Error<DeleteUserError>> {
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Delete,
    )
    .await?;

    let user = UserFull::retrieve(&transaction, &users_id)
        .await?
        .ok_or(Error::Inner(DeleteUserError::UserNotFound))?;

    let _groups = transaction
        .execute("delete from group_users where users_id = $1", &[&user.id])
        .await?;

    let _roles = transaction
        .execute("delete from user_roles where users_id = $1", &[&user.id])
        .await?;

    let _sessions = transaction
        .execute(
            "delete from authn_sessions where users_id = $1",
            &[&user.id],
        )
        .await?;

    let _totp = transaction
        .execute("delete from authn_totp where users_id = $1", &[&user.id])
        .await?;

    // need to do something with the journals that the user owns
    // as the most costly part will be removing any files

    let _user = transaction
        .execute("delete from users where id = $1", &[&user.id])
        .await?;

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
