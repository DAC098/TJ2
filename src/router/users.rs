use std::fmt::Write;

use axum::extract::{Request, Path};
use axum::http::{HeaderMap, Uri, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::StreamExt;
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
    added: DateTime<Utc>,
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

    let group_users = GroupUser::retrieve_users_id_stream(
        &conn,
        &user.id
    )
        .await
        .context("failed to retireve attached groups")?;

    futures::pin_mut!(group_users);

    let mut groups = Vec::new();

    while let Some(result) = group_users.next().await {
        let group_user = result.context("failed to retrieve attached group record")?;

        groups.push(AttachedGroup {
            groups_id: group_user.groups_id,
            added: group_user.added
        });
    }

    Ok(body::Json(UserFull {
        id: user.id,
        uid: user.uid,
        username: user.username,
        groups
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
    GroupsNotFound,
    Created(UserFull)
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
        let mut first = true;
        let mut rtn = Vec::new();
        let mut params: db::ParamsVec<'_> = vec![&user.id, &added];
        let mut query = String::from(
            "insert into group_users (groups_id, users_id, added) values "
        );

        for groups_id in groups {
            if first {
                first = false;
            } else {
                query.push_str(", ");
            }

            write!(
                &mut query,
                "(${}, $1, $2)",
                db::push_param(&mut params, groups_id)
            ).unwrap();

            rtn.push(AttachedGroup {
                groups_id: *groups_id,
                added
            });
        }

        query.push_str(" on conflict on constraint group_users_pkey do nothing");

        if let Err(err) = transaction.execute(query.as_str(), params.as_slice()).await {
            if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::ForeignKey(name) => if name == "group_users_groups_id_fkey" {
                        return Ok((
                            StatusCode::BAD_REQUEST,
                            body::Json(NewUserResult::GroupsNotFound)
                        ).into_response())
                    },
                    _ => {}
                }
            }

            return Err(error::Error::context_source(
                "failed to add groups to user", err
            ));
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
