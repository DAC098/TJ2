use axum::extract::{Request, Path};
use axum::http::{HeaderMap, Uri, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{UserId, UserUid, GroupId, RoleId};
use crate::error::{self, Context};
use crate::router::body;
use crate::router::macros;
use crate::state;
use crate::sec::{password, authz};
use crate::sec::authz::{AttachedRole, create_attached_roles, update_attached_roles};
use crate::user::{User, UserBuilder, UserBuilderError};
use crate::user::group::{AttachedGroup, create_attached_groups, update_attached_groups};

#[derive(Debug, Serialize)]
pub struct UserPartial {
    id: UserId,
    uid: UserUid,
    username: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
}

pub async fn retrieve_users(
    state: state::SharedState,
    req: Request,
) -> Result<Response, error::Error> {
    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator!(
        &conn,
        req.headers(),
        Some(req.uri().clone())
    );

    macros::res_if_html!(state.templates(), req.headers());

    let perm_check = authz::has_permission(
        &conn,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Read
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let params: db::ParamsArray<'_, 0> = [];
    let users = conn.query_raw(
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
        params
    )
        .await
        .context("failed to retrieve users")?;

    futures::pin_mut!(users);

    let mut found = Vec::new();

    while let Some(result) = users.next().await {
        let record = result.context("failed to retrieve user record")?;

        found.push(UserPartial {
            id: record.get(0),
            uid: record.get(1),
            username: record.get(2),
            created: record.get(3),
            updated: record.get(4),
        });
    }

    Ok(body::Json(found).into_response())
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
    ) -> Result<Option<Self>, error::Error> {
        let result = User::retrieve_id(conn, *users_id)
            .await
            .context("failed to retrieve user")?;

        let Some(user) = result else {
            return Ok(None);
        };

        let attached = tokio::join!(
            AttachedGroup::retrieve(conn, &user),
            AttachedRole::retrieve(conn, &user),
        );

        match attached {
            (Ok(groups), Ok(roles)) => Ok(Some(Self {
                id: user.id,
                uid: user.uid,
                username: user.username,
                created: user.created,
                updated: user.updated,
                groups,
                roles,
            })),
            (Ok(_), Err(err)) => Err(error::Error::context_source(
                "failed to retrieve user roles",
                err
            )),
            (Err(err), Ok(_)) => Err(error::Error::context_source(
                "failed to retrieve user groups",
                err
            )),
            (Err(g_err), Err(_r_err)) => Err(error::Error::context_source(
                "failed to retrieve user roles and groups",
                g_err
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct MaybeUserId {
    users_id: Option<UserId>
}

pub async fn retrieve_user(
    state: state::SharedState,
    headers: HeaderMap,
    uri: Uri,
    Path(MaybeUserId { users_id }): Path<MaybeUserId>,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let Some(users_id) = users_id else {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator!(
        &conn,
        &headers,
        Some(uri.clone())
    );

    let perm_check = authz::has_permission(
        &conn,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Read,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check  {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = UserFull::retrieve(&conn, &users_id)
        .await
        .context("failed to retrieve user")?;

    if let Some(user) = result {
        Ok(body::Json(user).into_response())
    } else {
        Ok(StatusCode::NOT_FOUND.into_response())
    }
}

#[derive(Debug, Deserialize)]
pub struct NewUser {
    username: String,
    password: String,
    groups: Vec<GroupId>,
    roles: Vec<RoleId>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum NewUserResult {
    UsernameExists,
    GroupsNotFound {
        ids: Vec<GroupId>,
    },
    RolesNotFound {
        ids: Vec<RoleId>,
    },
    Created(UserFull),
}

pub async fn create_user(
    db::Conn(mut conn): db::Conn,
    storage: state::Storage,
    headers: HeaderMap,
    body::Json(json): body::Json<NewUser>,
) -> Result<Response, error::Error> {
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator!(
        &transaction,
        &headers,
        None::<&str>
    );

    let perm_check = authz::has_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Create,
    )
        .await
        .context("failed to retrieve permision for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let builder = match UserBuilder::new_password(json.username, json.password) {
        Ok(b) => b,
        Err(err) => match err {
            UserBuilderError::Argon(_argon_err) => return Err(error::Error::context(
                "failed to hash new user password"
            )),
            _ => unreachable!()
        }
    };

    let user = match builder.build(&transaction).await {
        Ok(user) => user,
        Err(err) => match err {
            UserBuilderError::UsernameExists => return Ok((
                StatusCode::BAD_REQUEST,
                body::Json(NewUserResult::UsernameExists)
            ).into_response()),
            UserBuilderError::UidExists => return Err(error::Error::context(
                "user uid collision"
            )),
            UserBuilderError::Db(db_err) => return Err(error::Error::context_source(
                "failed to create new user",
                db_err
            )),
            _ => unreachable!()
        }
    };

    let (groups, not_found) = create_attached_groups(&transaction, &user, json.groups).await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewUserResult::GroupsNotFound {
                ids: not_found
            })
        ).into_response());
    }

    let (roles, not_found) = create_attached_roles(&transaction, &user, json.roles).await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewUserResult::RolesNotFound {
                ids: not_found
            })
        ).into_response());
    }

    let user_dir = storage.user_dir(user.id);

    user_dir.create()
        .await
        .context("failed to create user directory")?;

    let private_key = tj2_lib::sec::pki::PrivateKey::generate()
        .context("failed to generate private key")?;

    private_key.save(user_dir.private_key(), false)
        .await
        .context("failed to save private key")?;

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(body::Json(NewUserResult::Created(UserFull {
        id: user.id,
        uid: user.uid,
        username: user.username,
        created: user.created,
        updated: user.updated,
        groups,
        roles,
    })).into_response())
}

#[derive(Debug, Deserialize)]
pub struct UpdateUser {
    username: Option<String>,
    password: Option<String>,
    groups: Option<Vec<GroupId>>,
    roles: Option<Vec<RoleId>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum UpdatedUserResult {
    UsernameExists,
    GroupsNotFound {
        ids: Vec<GroupId>
    },
    RolesNotFound {
        ids: Vec<RoleId>,
    }
}

pub async fn update_user(
    db::Conn(mut conn): db::Conn,
    headers: HeaderMap,
    Path(UserPath { users_id }): Path<UserPath>,
    body::Json(json): body::Json<UpdateUser>,
) -> Result<Response, error::Error> {
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator!(
        &transaction,
        &headers,
        None::<&str>
    );

    let perm_check = authz::has_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Update
    )
        .await
        .context("failed to retrie user permission")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = User::retrieve_id(&transaction, users_id)
        .await
        .context("failed to retrieve user")?;

    let Some(mut user) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    if json.username.is_some() || json.password.is_some() {
        if let Some(username) = json.username {
            user.username = username;
        }

        if let Some(password) = &json.password {
            user.password = password::create(password)
                .context("failed to hash password for user")?;
            user.version = 0;
        }

        let result = user.update(&transaction)
            .await
            .context("failed to update user")?;

        if !result {
            return Ok((
                StatusCode::BAD_REQUEST,
                body::Json(UpdatedUserResult::UsernameExists)
            ).into_response());
        }
    }

    let (_attached, not_found) = update_attached_groups(&transaction, &user, json.groups)
        .await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdatedUserResult::GroupsNotFound {
                ids: not_found
            })
        ).into_response());
    }

    let (_attached, not_found) = update_attached_roles(&transaction, &user, json.roles)
        .await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdatedUserResult::RolesNotFound {
                ids: not_found
            })
        ).into_response())
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(StatusCode::OK.into_response())
}

pub async fn delete_user(
    db::Conn(mut conn): db::Conn,
    headers: HeaderMap,
    Path(UserPath { users_id }): Path<UserPath>
) -> Result<Response, error::Error> {
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator!(
        &transaction,
        &headers,
        None::<&str>
    );

    let perm_check = authz::has_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Users,
        authz::Ability::Delete,
    )
        .await
        .context("failed to retrieve permision for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = UserFull::retrieve(&transaction, &users_id)
        .await
        .context("failed to retrieve user")?;

    let Some(user) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let _groups = transaction.execute(
        "delete from group_users where users_id = $1",
        &[&user.id]
    )
        .await
        .context("failed to delete from group users")?;

    let _roles = transaction.execute(
        "delete from user_roles where users_id = $1",
        &[&user.id]
    )
        .await
        .context("failed to delete from user roles")?;

    let _sessions = transaction.execute(
        "delete from authn_sessions where users_id = $1",
        &[&user.id]
    )
        .await
        .context("failed to delete from authn sessions")?;

    let _totp = transaction.execute(
        "delete from authn_totp where users_id = $1",
        &[&user.id]
    )
        .await
        .context("failed to delete from authn totp")?;

    // need to do something with the journals that the user owns
    // as the most costly part will be removing any files

    let _user = transaction.execute(
        "delete from users where id = $1",
        &[&user.id]
    )
        .await
        .context("failed to delete from users")?;

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(StatusCode::OK.into_response())
}
