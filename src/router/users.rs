use std::collections::HashSet;
use std::fmt::Write;

use axum::extract::{Request, Path};
use axum::http::{HeaderMap, Uri, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{UserId, UserUid, GroupId};
use crate::error::{self, Context};
use crate::router::body;
use crate::router::macros;
use crate::state;
use crate::sec::{password, authz};
use crate::user::{User, GroupUser};

#[derive(Debug, Serialize)]
pub struct UserPartial {
    id: UserId,
    uid: UserUid,
    username: String,
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
               search_users.username \
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
        });
    }

    Ok(body::Json(found).into_response())
}

#[derive(Debug, Deserialize)]
pub struct UserPath {
    user_id: UserId,
}

#[derive(Debug, Serialize)]
pub struct UserFull {
    id: UserId,
    uid: UserUid,
    username: String,
    groups: Vec<AttachedGroup>
}

#[derive(Debug, Serialize)]
pub struct AttachedGroup {
    groups_id: GroupId,
    name: String,
    added: DateTime<Utc>,
}

impl AttachedGroup {
    async fn retrieve_stream(
        conn: &impl db::GenericClient,
        users_id: &UserId
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [users_id];

        let stream = conn.query_raw(
            "\
            select group_users.groups_id, \
                   groups.name, \
                   group_users.added \
            from group_users \
                left join groups on \
                    group_users.groups_id = groups.id \
            where group_users.users_id = $1",
            params
        ).await?;

        Ok(stream.map(|result| result.map(|row| Self {
            groups_id: row.get(0),
            name: row.get(1),
            added: row.get(2),
        })))
    }

    async fn retrieve(
        conn: &impl db::GenericClient,
        users_id: &UserId
    ) -> Result<Vec<Self>, error::Error> {
        let stream = Self::retrieve_stream(conn, users_id)
            .await
            .context("failed to retrieve attached groups")?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(result) = stream.next().await {
            let record = result.context("failed to retrieve attached group record")?;

            rtn.push(record);
        }

        Ok(rtn)
    }
}

pub async fn retrieve_user(
    state: state::SharedState,
    headers: HeaderMap,
    uri: Uri,
    Path(UserPath { user_id }): Path<UserPath>,
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
        authz::Scope::Users,
        authz::Ability::Read,
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check  {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = User::retrieve_id(&conn, user_id)
        .await
        .context("failed to retrieve user")?;

    let Some(user) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let groups = AttachedGroup::retrieve(&conn, &user.id).await?;

    Ok(body::Json(UserFull {
        id: user.id,
        uid: user.uid,
        username: user.username,
        groups,
    }).into_response())
}

#[derive(Debug, Deserialize)]
pub struct NewUser {
    username: String,
    password: String,
    groups: Option<Vec<GroupId>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum NewUserResult {
    UsernameExists,
    GroupsNotFound {
        ids: Vec<GroupId>
    },
    Created(UserFull),
}

pub async fn create_user(
    db::Conn(mut conn): db::Conn,
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

    let hashed = password::create(&json.password)
        .context("failed to hash new user password")?;

    let result = User::create(&transaction, &json.username, &hashed, 0)
        .await
        .context("failed to create new user")?;

    let Some(user) = result else {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewUserResult::UsernameExists)
        ).into_response())
    };

    let groups = if let Some(groups) = &json.groups {
        let added = Utc::now();
        let params: db::ParamsArray<'_, 3> = [&user.id, &added, &groups];
        let mut requested: HashSet<GroupId> = HashSet::from_iter(groups.clone());
        let mut rtn = Vec::new();

        let stream = transaction.query_raw(
            "\
            with tmp_insert as ( \
                insert into group_users (groups_id, users_id, added) \
                select groups.id, \
                       $1::bigint as users_id, \
                       $2::timestamp with time zone as added \
                from groups \
                where groups.id = any($3) \
                returning * \
            ) \
            select tmp_insert.groups_id, \
                   groups.name, \
                   tmp_insert.added \
            from tmp_insert \
                left join groups on \
                    tmp_insert.groups_id = groups.id",
            params
        )
            .await
            .context("failed to add groups to user")?;

        futures::pin_mut!(stream);

        while let Some(result) = stream.next().await {
            let record = result.context("failed to retrieve added group")?;
            let groups_id = record.get(0);

            if !requested.remove(&groups_id) {
                tracing::warn!("a group was added that was not requested");
            }

            rtn.push(AttachedGroup {
                groups_id,
                name: record.get(1),
                added: record.get(2),
            });
        }

        if !requested.is_empty() {
            return Ok((
                StatusCode::BAD_REQUEST,
                body::Json(NewUserResult::GroupsNotFound {
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

    Ok(body::Json(NewUserResult::Created(UserFull {
        id: user.id,
        uid: user.uid,
        username: user.username,
        groups,
    })).into_response())
}
