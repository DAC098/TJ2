use std::fmt::Write;

use axum::extract::{Request, Path};
use axum::http::{HeaderMap, Uri, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{UserId, GroupId, GroupUid};
use crate::error::{self, Context};
use crate::router::body;
use crate::router::macros;
use crate::state;
use crate::sec::authz;
use crate::user::{Group, GroupUser};

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

#[derive(Debug, Serialize)]
pub struct GroupFull {
    id: GroupId,
    uid: GroupUid,
    name: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
    users: Vec<AttachedUser>,
}

#[derive(Debug, Serialize)]
pub struct AttachedUser {
    users_id: UserId,
    added: DateTime<Utc>
}

pub async fn retrieve_group(
    state: state::SharedState,
    headers: HeaderMap,
    uri: Uri,
    Path(GroupPath { groups_id }): Path<GroupPath>
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
        authz::Scope::Groups,
        authz::Ability::Read,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = Group::retrieve_id(&conn, groups_id)
        .await
        .context("failed to retrieve group")?;

    let Some(group) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let group_users = GroupUser::retrieve_groups_id_stream(
        &conn,
        &group.id
    )
        .await
        .context("failed to retrieve attached users")?;

    futures::pin_mut!(group_users);

    let mut users = Vec::new();

    while let Some(result) = group_users.next().await {
        let group_user = result.context("failed to retrieve attached user record")?;

        users.push(AttachedUser {
            users_id: group_user.users_id,
            added: group_user.added
        });
    }

    Ok(body::Json(GroupFull {
        id: group.id,
        uid: group.uid,
        name: group.name,
        created: group.created,
        updated: group.updated,
        users
    }).into_response())
}

#[derive(Debug, Deserialize)]
pub struct NewGroup {
    name: String,
    users: Option<Vec<UserId>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "result")]
pub enum NewGroupResult {
    GroupExists,
    UsersNotFound,
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

    let users = if let Some(users) = &json.users {
        let added = Utc::now();
        let mut first = true;
        let mut rtn = Vec::new();
        let mut params: db::ParamsVec<'_> = vec![&group.id, &added];
        let mut query = String::from(
            "insert into group_users (groups_id, users_id, added) values "
        );

        for user_id in users {
            if first {
                first = false;
            } else {
                query.push_str(", ");
            }

            write!(
                &mut query,
                "($1, ${}, $2)",
                db::push_param(&mut params, user_id)
            ).unwrap();

            rtn.push(AttachedUser {
                users_id: *user_id,
                added
            });
        }

        query.push_str(" on conflict on constraint group_users_pkey do nothing");

        if let Err(err) = transaction.execute(query.as_str(), params.as_slice()).await {
            if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::ForeignKey(name) => if name == "group_users_users_id_fkey" {
                        return Ok((
                            StatusCode::BAD_REQUEST,
                            body::Json(NewGroupResult::UsersNotFound)
                        ).into_response());
                    }
                    _ => {}
                }
            }

            return Err(error::Error::context_source(
                "failed to add users to group", err
            ));
        }

        rtn
    } else {
        Vec::new()
    };

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(body::Json(NewGroupResult::Created(GroupFull {
        id: group.id,
        uid: group.uid,
        name: group.name,
        created: group.created,
        updated: group.updated,
        users
    })).into_response())
}
