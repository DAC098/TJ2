use axum::extract::{Request, Path};
use axum::http::{HeaderMap, Uri, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{UserId, GroupId, GroupUid, RoleId};
use crate::error::{self, Context};
use crate::router::body;
use crate::router::macros;
use crate::state;
use crate::sec::authz::{self, AttachedRole, create_attached_roles, update_attached_roles};
use crate::user::{AttachedUser, create_attached_users, update_attached_users};
use crate::user::group::Group;

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
        authz::Scope::Groups,
        authz::Ability::Read,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let params: db::ParamsArray<'_, 0> = [];
    let groups = conn.query_raw(
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
        params
    )
        .await
        .context("failed to retrieve groups")?;

    futures::pin_mut!(groups);

    let mut rtn = Vec::new();

    while let Some(result) = groups.next().await {
        let record = result.context("failed to retrieve group record")?;

        rtn.push(GroupPartial {
            id: record.get(0),
            uid: record.get(1),
            name: record.get(2),
            created: record.get(3),
            updated: record.get(4),
        });
    }

    Ok(body::Json(rtn).into_response())
}

#[derive(Debug, Deserialize)]
pub struct GroupPath {
    groups_id: GroupId,
}

#[derive(Debug, Deserialize)]
pub struct MaybeGroupPath {
    groups_id: Option<GroupId>,
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
        groups_id: &GroupId
    ) -> Result<Option<Self>, error::Error> {
        let result = Group::retrieve_id(conn, *groups_id)
            .await
            .context("failed to retrieve group")?;

        let Some(group) = result else {
            return Ok(None);
        };

        let results = tokio::join!(
            AttachedUser::retrieve(conn, &group),
            AttachedRole::retrieve(conn, &group)
        );

        match results {
            (Ok(users), Ok(roles)) => Ok(Some(Self {
                id: group.id,
                uid: group.uid,
                name: group.name,
                created: group.created,
                updated: group.updated,
                users,
                roles,
            })),
            (Err(err), Ok(_)) => Err(error::Error::context_source(
                "failed to retrieve users",
                err
            )),
            (Ok(_), Err(err)) => Err(error::Error::context_source(
                "failed to retrieve roles",
                err
            )),
            (Err(u_err), Err(_r_err)) => Err(error::Error::context_source(
                "failed to retrieve users and roles",
                u_err
            ))
        }
    }
}

pub async fn retrieve_group(
    state: state::SharedState,
    headers: HeaderMap,
    uri: Uri,
    Path(MaybeGroupPath { groups_id }): Path<MaybeGroupPath>
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let Some(groups_id) = groups_id else {
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
        authz::Scope::Groups,
        authz::Ability::Read,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let Some(group) = GroupFull::retrieve(&conn, &groups_id).await? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    Ok(body::Json(group).into_response())
}

#[derive(Debug, Deserialize)]
pub struct NewGroup {
    name: String,
    users: Vec<UserId>,
    roles: Vec<RoleId>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "result")]
pub enum NewGroupResult {
    GroupExists,
    UsersNotFound {
        ids: Vec<UserId>
    },
    RolesNotFound {
        ids: Vec<RoleId>
    },
    Created(GroupFull)
}

pub async fn create_group(
    db::Conn(mut conn): db::Conn,
    headers: HeaderMap,
    body::Json(json): body::Json<NewGroup>
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
        authz::Scope::Groups,
        authz::Ability::Create,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = Group::create(&transaction, &json.name)
        .await
        .context("failed to create new group")?;

    let Some(group) = result else {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewGroupResult::GroupExists)
        ).into_response())
    };

    let (users, not_found) = create_attached_users(&transaction, &group, json.users)
        .await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewGroupResult::UsersNotFound {
                ids: not_found
            })
        ).into_response());
    }

    let (roles, not_found) = create_attached_roles(&transaction, &group, json.roles)
        .await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewGroupResult::RolesNotFound {
                ids: not_found
            })
        ).into_response());
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(body::Json(NewGroupResult::Created(GroupFull {
        id: group.id,
        uid: group.uid,
        name: group.name,
        created: group.created,
        updated: group.updated,
        users,
        roles,
    })).into_response())
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroup {
    name: Option<String>,
    users: Option<Vec<UserId>>,
    roles: Option<Vec<RoleId>>,
}


#[derive(Debug, Serialize)]
#[serde(tag = "result")]
pub enum UpdateGroupResult {
    GroupExists,
    UsersNotFound {
        ids: Vec<UserId>
    },
    RolesNotFound {
        ids: Vec<RoleId>
    },
}

pub async fn update_group(
    db::Conn(mut conn): db::Conn,
    headers: HeaderMap,
    Path(GroupPath { groups_id }): Path<GroupPath>,
    body::Json(json): body::Json<UpdateGroup>,
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
        authz::Scope::Groups,
        authz::Ability::Update,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = Group::retrieve_id(&transaction, groups_id)
        .await
        .context("failed to retrieve group")?;

    let Some(mut group) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    if json.name.is_some() {
        if let Some(name) = json.name {
            group.name = name;
        }

        let did_update = group.update(&transaction)
            .await
            .context("failed to update group")?;

        if !did_update {
            return Ok((
                StatusCode::BAD_REQUEST,
                body::Json(UpdateGroupResult::GroupExists)
            ).into_response());
        }
    }

    let (_attached, not_found) = update_attached_users(&transaction, &group, json.users)
        .await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdateGroupResult::UsersNotFound {
                ids: not_found
            })
        ).into_response());
    }

    let (_attached, not_found) = update_attached_roles(&transaction, &group, json.roles)
        .await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdateGroupResult::RolesNotFound {
                ids: not_found
            })
        ).into_response());
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(StatusCode::OK.into_response())
}

pub async fn delete_group(
    db::Conn(mut conn): db::Conn,
    headers: HeaderMap,
    Path(GroupPath { groups_id }): Path<GroupPath>
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
        authz::Scope::Groups,
        authz::Ability::Delete,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = GroupFull::retrieve(&transaction, &groups_id)
        .await
        .context("failed to retrieve group")?;

    let Some(group) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let _users = transaction.execute(
        "delete from group_users where groups_id = $1",
        &[&group.id]
    )
        .await
        .context("failed to delete from group users")?;

    let _roles = transaction.execute(
        "delete from group_roles where groups_id = $1",
        &[&group.id]
    )
        .await
        .context("failed to delete from group roles")?;

    let _user = transaction.execute(
        "delete from groups where id = $1",
        &[&group.id]
    )
        .await
        .context("failed to delete from groups")?;

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(StatusCode::OK.into_response())
}
