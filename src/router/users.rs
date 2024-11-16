use std::collections::{HashMap, HashSet};

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
use crate::user::User;

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
    users_id: UserId,
}

#[derive(Debug, Serialize)]
pub struct UserFull {
    id: UserId,
    uid: UserUid,
    username: String,
    groups: Vec<AttachedGroup>
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

        let groups = AttachedGroup::retrieve(conn, &user.id).await?;

        Ok(Some(UserFull {
            id: user.id,
            uid: user.uid,
            username: user.username,
            groups,
        }))
    }
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
    Path(UserPath { users_id }): Path<UserPath>,
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

    let (groups, not_found) = create_groups(&transaction, &user, json.groups).await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewUserResult::GroupsNotFound {
                ids: not_found
            })
        ).into_response());
    }

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

#[derive(Debug, Deserialize)]
pub struct UpdateUser {
    username: Option<String>,
    password: Option<String>,
    groups: Option<Vec<GroupId>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum UpdatedUserResult {
    UsernameExists,
    GroupsNotFound {
        ids: Vec<GroupId>
    },
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

    let (_attached, not_found) = update_groups(&transaction, &user, json.groups)
        .await?;

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdatedUserResult::GroupsNotFound {
                ids: not_found
            })
        ).into_response());
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

fn unique_groups<V>(
    groups: Vec<GroupId>,
    current: Option<&HashMap<GroupId, V>>,
) -> (HashSet<GroupId>, Vec<GroupId>, bool) {
    let mut set = HashSet::with_capacity(groups.len());
    let mut list = Vec::with_capacity(groups.len());

    if let Some(current) = current {
        let mut diff = false;

        for id in groups {
            set.insert(id);
            list.push(id);

            if !current.contains_key(&id) {
                diff = true;
            }
        }

        (set, list, diff)
    } else {
        (set, list, true)
    }
}

async fn create_groups(
    conn: &impl db::GenericClient,
    user: &User,
    groups: Option<Vec<GroupId>>,
) -> Result<(Vec<AttachedGroup>, Vec<GroupId>), error::Error> {
    let Some(groups) = groups else {
        return Ok((Vec::new(), Vec::new()));
    }; 

    let added = Utc::now();
    let (mut requested, groups, _diff) = unique_groups::<()>(groups, None);

    let params: db::ParamsArray<'_, 3> = [&user.id, &added, &groups];
    let mut rtn = Vec::new();

    let stream = conn.query_raw(
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

    Ok((rtn, Vec::new()))
}

async fn update_groups(
    conn: &impl db::GenericClient,
    user: &User,
    groups: Option<Vec<GroupId>>
) -> Result<(Vec<AttachedGroup>, Vec<GroupId>), error::Error> {
    let Some(groups) = groups else {
        return Ok((
            AttachedGroup::retrieve(conn, &user.id).await?,
            Vec::new()
        ));
    };

    let added = Utc::now();
    let mut current: HashMap<GroupId, AttachedGroup> = HashMap::new();
    let stream = AttachedGroup::retrieve_stream(conn, &user.id)
        .await
        .context("failed to retrieve currently attached groups")?;

    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        let record = result.context("failed to retrieve current attached group")?;

        current.insert(record.groups_id, record);
    }

    let (mut requested, groups, diff) = unique_groups(groups, Some(&current));

    if !diff {
        return Ok((current.into_values().collect(), Vec::new()));
    }

    let params: db::ParamsArray<'_, 3> = [&user.id, &added, &groups];
    let mut rtn = Vec::new();

    let stream = conn.query_raw(
        "\
        with tmp_insert as ( \
            insert into group_users (groups_id, users_id, added) \
            select groups.id, \
                   $1::bigint as users_id, \
                   $2::timestamp with time zone as added \
            from groups \
            where groups.id = any($3) \
            on conflict on constraint group_users_pkey do nothing \
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

        current.remove(&groups_id);

        rtn.push(AttachedGroup {
            groups_id,
            name: record.get(1),
            added: record.get(2),
        });
    }

    if !current.is_empty() {
        let to_delete = Vec::from_iter(current.into_keys());

        conn.execute(
            "delete from group_users where groups_id = any($1)",
            &[&to_delete]
        )
            .await
            .context("failed to delete from groups users")?;
    }

    Ok((rtn, Vec::from_iter(requested)))
}
