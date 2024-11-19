use axum::extract::{Request, Path};
use axum::http::{HeaderMap, Uri, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{UserId, GroupId, RoleId, RoleUid};
use crate::error::{self, Context};
use crate::router::body;
use crate::router::macros;
use crate::state;
use crate::sec::authz::{self, Role};
use crate::user::{AttachedUser, AttachedGroup, create_attached_users, update_attached_users, create_attached_groups, update_attached_groups};

#[derive(Debug, Serialize)]
pub struct RolePartial {
    id: RoleId,
    uid: RoleUid,
    name: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
}

pub async fn retrieve_roles(
    state: state::SharedState,
    req: Request
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
        authz::Scope::Roles,
        authz::Ability::Read,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let params: db::ParamsArray<'_, 0> = [];
    let roles = conn.query_raw(
        "\
        with search_roles as ( \
            select * from authz_roles\
        ) \
        select search_roles.id, \
               search_roles.uid, \
               search_roles.name, \
               search_roles.created, \
               search_roles.updated \
        from search_roles \
        order by search_roles.name",
        params
    )
        .await
        .context("failed to retrieve roles")?;

    futures::pin_mut!(roles);

    let mut found = Vec::new();

    while let Some(result) = roles.next().await {
        let record = result.context("failed to retrieve role record")?;

        found.push(RolePartial {
            id: record.get(0),
            uid: record.get(1),
            name: record.get(2),
            created: record.get(3),
            updated: record.get(4),
        });
    }

    Ok(body::Json(found).into_response())
}

#[derive(Debug, Deserialize)]
pub struct RolePath {
    role_id: RoleId
}

#[derive(Debug, Serialize)]
pub struct RoleFull {
    id: RoleId,
    uid: RoleUid,
    name: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
    users: Vec<AttachedUser>,
    groups: Vec<AttachedGroup>,
}

impl RoleFull {
    pub async fn retrieve(conn: &impl db::GenericClient, role_id: &RoleId) -> Result<Option<Self>, error::Error> {
        let result = Role::retrieve_id(conn, role_id)
            .await
            .context("failed to retrieve role")?;

        let Some(role) = result else {
            return Ok(None);
        };

        let result = tokio::join!(
            AttachedUser::retrieve(conn, &role),
            AttachedGroup::retrieve(conn, &role),
        );

        match result {
            (Ok(users), Ok(groups)) => Ok(Some(Self {
                id: role.id,
                uid: role.uid,
                name: role.name,
                created: role.created,
                updated: role.updated,
                users,
                groups
            })),
            (Err(err), Ok(_)) => Err(error::Error::context_source(
                "failed to retrieve users",
                err
            )),
            (Ok(_), Err(err)) => Err(error::Error::context_source(
                "failed to retrieve groups",
                err
            )),
            (Err(u_err), Err(_g_err)) => Err(error::Error::context_source(
                "failed to retrieve users and groups",
                u_err
            ))
        }
    }
}

pub async fn retrieve_role(
    state: state::SharedState,
    headers: HeaderMap,
    uri: Uri,
    Path(RolePath { role_id }): Path<RolePath>
) -> Result<Response, error::Error> {
    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator!(
        &conn,
        &headers,
        Some(uri.clone())
    );

    macros::res_if_html!(state.templates(), &headers);

    let perm_check = authz::has_permission(
        &conn,
        initiator.user.id,
        authz::Scope::Roles,
        authz::Ability::Read,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = RoleFull::retrieve(&conn, &role_id)
        .await
        .context("failed to retrieve role")?;

    if let Some(role) = result {
        Ok(body::Json(role).into_response())
    } else {
        Ok(StatusCode::NOT_FOUND.into_response())
    }
}

#[derive(Debug, Deserialize)]
pub struct NewRole {
    name: String,
    users: Vec<UserId>,
    groups: Vec<GroupId>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "result")]
pub enum NewRoleResult {
    RoleExists,
    UsersNotFound {
        ids: Vec<UserId>
    },
    GroupsNotFound {
        ids: Vec<GroupId>
    },
    Created(RoleFull)
}

pub async fn create_role(
    db::Conn(mut conn): db::Conn,
    headers: HeaderMap,
    body::Json(json): body::Json<NewRole>,
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
        authz::Scope::Roles,
        authz::Ability::Create,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = Role::create(&transaction, &json.name)
        .await
        .context("failed to create new role")?;

    let Some(role) = result else {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewRoleResult::RoleExists)
        ).into_response());
    };

    let (users, not_found) = create_attached_users(&transaction, &role, json.users).await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewRoleResult::UsersNotFound {
                ids: not_found
            })
        ).into_response());
    }

    let (groups, not_found) = create_attached_groups(&transaction, &role, json.groups).await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewRoleResult::GroupsNotFound {
                ids: not_found
            })
        ).into_response());
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(body::Json(NewRoleResult::Created(RoleFull {
        id: role.id,
        uid: role.uid,
        name: role.name,
        created: role.created,
        updated: role.updated,
        users,
        groups
    })).into_response())
}

#[derive(Debug, Deserialize)]
pub struct UpdateRole {
    name: Option<String>,
    users: Option<Vec<UserId>>,
    groups: Option<Vec<GroupId>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "result")]
pub enum UpdateRoleResult {
    RoleExists,
    UsersNotFound {
        ids: Vec<UserId>
    },
    GroupsNotFound {
        ids: Vec<GroupId>
    }
}

pub async fn update_role(
    mut conn: db::Conn,
    headers: HeaderMap,
    Path(RolePath { role_id }): Path<RolePath>,
    body::Json(json): body::Json<UpdateRole>,
) -> Result<Response, error::Error> {
    let transaction = conn.transaction().await?;

    let initiator = macros::require_initiator!(
        &transaction,
        &headers,
        None::<&str>
    );

    let perm_check = authz::has_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Roles,
        authz::Ability::Update
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = Role::retrieve_id(&transaction, &role_id)
        .await
        .context("failed to retrieve role")?;

    let Some(mut role) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    if json.name.is_some() {
        if let Some(name) = json.name {
            role.name = name;
        }

        let did_update = role.update(&transaction)
            .await
            .context("failed to update role")?;

        if !did_update {
            return Ok((
                StatusCode::BAD_REQUEST,
                body::Json(UpdateRoleResult::RoleExists)
            ).into_response());
        }
    }

    let (_attached, not_found) = update_attached_users(&transaction, &role, json.users).await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdateRoleResult::UsersNotFound {
                ids: not_found
            })
        ).into_response());
    }

    let (_attached, not_found) = update_attached_groups(&transaction, &role, json.groups).await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdateRoleResult::GroupsNotFound {
                ids: not_found
            })
        ).into_response());
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(StatusCode::OK.into_response())
}

pub async fn delete_role(
    mut conn: db::Conn,
    headers: HeaderMap,
    Path(RolePath { role_id }): Path<RolePath>,
) -> Result<Response, error::Error> {
    let transaction = conn.transaction().await?;

    let initiator = macros::require_initiator!(
        &transaction,
        &headers,
        None::<&str>
    );

    let perm_check = authz::has_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Roles,
        authz::Ability::Delete,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = Role::retrieve_id(&transaction, &role_id)
        .await
        .context("failed to retrieve role")?;

    let Some(role) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let _users = transaction.execute(
        "delete from user_roles where role_id = $1",
        &[&role.id]
    )
        .await
        .context("failed to delete frome user roles")?;

    let _groups = transaction.execute(
        "delete from group_roles where role_id = $1",
        &[&role.id]
    )
        .await
        .context("failed to delete from group roles")?;

    let _permissions = transaction.execute(
        "delete from authz_permissions where role_id = $1",
        &[&role.id]
    )
        .await
        .context("failed to delete from authz permissions")?;

    let _role = transaction.execute(
        "delete from authz_roles where id = $1",
        &[&role.id]
    )
        .await
        .context("failed to delete from authz roles")?;

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(StatusCode::OK.into_response())
}
