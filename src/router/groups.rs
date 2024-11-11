use std::collections::HashSet;

use axum::extract::{Request, Path};
use axum::http::{HeaderMap, Uri, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{UserId, GroupId, GroupUid};
use crate::error::{self, Context};
use crate::router::body;
use crate::router::macros;
use crate::state;
use crate::sec::authz;
use crate::user::Group;

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
    username: String,
    added: DateTime<Utc>
}

impl AttachedUser {
    async fn retrieve_stream(
        conn: &impl db::GenericClient,
        groups_id: &GroupId,
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [groups_id];

        let stream = conn.query_raw(
            "\
            select group_users.users_id, \
                   users.username, \
                   group_users.added \
            from group_users \
                left join users on \
                    group_users.users_id = users.id \
            where group_users.users_id = $1",
            params
        ).await?;

        Ok(stream.map(|result| result.map(|row| Self {
            users_id: row.get(0),
            username: row.get(1),
            added: row.get(2),
        })))
    }

    async fn retrieve(
        conn: &impl db::GenericClient,
        groups_id: &GroupId
    ) -> Result<Vec<Self>, error::Error> {
        let stream = Self::retrieve_stream(conn, groups_id)
            .await
            .context("failed to retrieve attached users")?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(result) = stream.next().await {
            let record = result.context("failed to retrieve attached user record")?;

            rtn.push(record);
        }

        Ok(rtn)
    }
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

    let users = AttachedUser::retrieve(&conn, &group.id).await?;

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
    UsersNotFound {
        ids: Vec<UserId>
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

    let users = if let Some(users) = &json.users {
        let added = Utc::now();
        let params: db::ParamsArray<'_, 3> = [&group.id, &added, &users];
        let mut requested: HashSet<UserId> = HashSet::from_iter(users.clone());
        let mut rtn = Vec::new();

        let stream = transaction.query_raw(
            "\
            with tmp_insert as ( \
                insert into group_users (users_id, groups_id, added) \
                select users.id, \
                       $1::bigint as groups_id, \
                       $2::timestamp with time zone as added \
                from users \
                where users.id = any($3) \
                returning * \
            ) \
            select tmp_insert.users_id, \
                   users.username, \
                   tmp_insert.added \
            from tmp_insert \
                left join users on \
                    tmp_insert.users_id = users.id",
            params
        )
            .await
            .context("failed to add users to group")?;

        futures::pin_mut!(stream);

        while let Some(result) = stream.next().await {
            let record = result.context("failed to retrieve added user")?;
            let users_id = record.get(0);

            if !requested.remove(&users_id) {
                tracing::warn!("a user was added that was not requested");
            }

            rtn.push(AttachedUser {
                users_id,
                username: record.get(1),
                added: record.get(2),
            });
        }

        if !requested.is_empty() {
            return Ok((
                StatusCode::BAD_REQUEST,
                body::Json(NewGroupResult::UsersNotFound {
                    ids: Vec::from_iter(requested)
                })
            ).into_response());
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
