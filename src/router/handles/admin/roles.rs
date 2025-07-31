use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{GroupId, RoleId, RoleUid, UserId};
use crate::net::{body, Error};
use crate::sec::authn::Initiator;
use crate::sec::authz::{self, Role};
use crate::state;
use crate::user::group::{
    create_attached_groups, update_attached_groups, AttachedGroup, AttachedGroupError,
};
use crate::user::{create_attached_users, update_attached_users, AttachedUser, AttachedUserError};

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
    initiator: Initiator,
    headers: HeaderMap,
) -> Result<body::Json<Vec<RolePartial>>, Error> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db().get().await?;

    authz::assert_permission(
        &conn,
        initiator.user.id,
        authz::Scope::Roles,
        authz::Ability::Read,
    )
    .await?;

    let params: db::ParamsArray<'_, 0> = [];
    let roles = conn
        .query_raw(
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
            params,
        )
        .await?;

    futures::pin_mut!(roles);

    let mut found = Vec::new();

    while let Some(result) = roles.next().await {
        let record = result?;

        found.push(RolePartial {
            id: record.get(0),
            uid: record.get(1),
            name: record.get(2),
            created: record.get(3),
            updated: record.get(4),
        });
    }

    Ok(body::Json(found))
}

#[derive(Debug, Deserialize)]
pub struct RolePath {
    role_id: RoleId,
}

#[derive(Debug, Serialize)]
pub struct RoleFull {
    id: RoleId,
    uid: RoleUid,
    name: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
    permissions: Vec<AttachedPermission>,
    users: Vec<AttachedUser>,
    groups: Vec<AttachedGroup>,
}

impl RoleFull {
    pub async fn retrieve(
        conn: &impl db::GenericClient,
        role_id: &RoleId,
    ) -> Result<Option<Self>, db::PgError> {
        let result = Role::retrieve_id(conn, role_id).await?;

        let Some(role) = result else {
            return Ok(None);
        };

        let (users, groups, permissions) = tokio::join!(
            AttachedUser::retrieve(conn, &role),
            AttachedGroup::retrieve(conn, &role),
            AttachedPermission::retrieve(conn, &role.id),
        );

        Ok(Some(Self {
            id: role.id,
            uid: role.uid,
            name: role.name,
            created: role.created,
            updated: role.updated,
            permissions: permissions?,
            users: users?,
            groups: groups?,
        }))
    }
}

#[derive(Debug, Serialize)]
pub struct AttachedPermission {
    scope: authz::Scope,
    ability: authz::Ability,
    added: DateTime<Utc>,
}

impl AttachedPermission {
    async fn retrieve_stream(
        conn: &impl db::GenericClient,
        role_id: &RoleId,
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [role_id];

        let stream = conn
            .query_raw(
                "\
            select authz_permissions.scope, \
                   authz_permissions.ability, \
                   authz_permissions.added
            from authz_permissions \
            where authz_permissions.role_id = $1 \
            order by authz_permissions.scope, \
                     authz_permissions.ability",
                params,
            )
            .await?;

        Ok(stream.map(|result| {
            result.map(|row| Self {
                scope: row.get(0),
                ability: row.get(1),
                added: row.get(2),
            })
        }))
    }

    async fn retrieve(
        conn: &impl db::GenericClient,
        role_id: &RoleId,
    ) -> Result<Vec<Self>, db::PgError> {
        let stream = Self::retrieve_stream(conn, role_id).await?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(result) = stream.next().await {
            rtn.push(result?);
        }

        Ok(rtn)
    }
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum RetrieveRoleError {
    RoleNotFound,
}

impl IntoResponse for RetrieveRoleError {
    fn into_response(self) -> Response {
        match self {
            Self::RoleNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn retrieve_role(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(RolePath { role_id }): Path<RolePath>,
) -> Result<body::Json<RoleFull>, Error<RetrieveRoleError>> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db_conn().await?;

    authz::assert_permission(
        &conn,
        initiator.user.id,
        authz::Scope::Roles,
        authz::Ability::Read,
    )
    .await?;

    let role = RoleFull::retrieve(&conn, &role_id)
        .await?
        .ok_or(Error::Inner(RetrieveRoleError::RoleNotFound))?;

    Ok(body::Json(role))
}

#[derive(Debug, Deserialize)]
pub struct PermissionBody {
    scope: authz::Scope,
    abilities: Vec<authz::Ability>,
}

#[derive(Debug, Deserialize)]
pub struct NewRole {
    name: String,
    permissions: Vec<PermissionBody>,
    users: Vec<UserId>,
    groups: Vec<GroupId>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum NewRoleError {
    RoleExists,
    UsersNotFound { ids: Vec<UserId> },
    GroupsNotFound { ids: Vec<GroupId> },
}

impl IntoResponse for NewRoleError {
    fn into_response(self) -> Response {
        match self {
            Self::RoleExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::UsersNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::GroupsNotFound { .. } => {
                (StatusCode::NOT_FOUND, body::Json(self)).into_response()
            }
        }
    }
}

pub async fn create_role(
    db::Conn(mut conn): db::Conn,
    initiator: Initiator,
    body::Json(json): body::Json<NewRole>,
) -> Result<body::Json<RoleFull>, Error<NewRoleError>> {
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Roles,
        authz::Ability::Create,
    )
    .await?;

    let role = Role::create(&transaction, &json.name)
        .await?
        .ok_or(Error::Inner(NewRoleError::RoleExists))?;

    let permissions = create_permissions(&transaction, &role, json.permissions).await?;

    let users = match create_attached_users(&transaction, &role, json.users).await {
        Ok(users) => users,
        Err(err) => {
            return Err(match err {
                AttachedUserError::NotFound(ids) => {
                    Error::Inner(NewRoleError::UsersNotFound { ids })
                }
                AttachedUserError::Db(err) => Error::from(err),
            })
        }
    };

    let groups = match create_attached_groups(&transaction, &role, json.groups).await {
        Ok(groups) => groups,
        Err(err) => {
            return Err(match err {
                AttachedGroupError::NotFound(ids) => {
                    Error::Inner(NewRoleError::GroupsNotFound { ids })
                }
                AttachedGroupError::Db(err) => Error::from(err),
            })
        }
    };

    transaction.commit().await?;

    Ok(body::Json(RoleFull {
        id: role.id,
        uid: role.uid,
        name: role.name,
        created: role.created,
        updated: role.updated,
        permissions,
        users,
        groups,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UpdateRole {
    name: Option<String>,
    users: Option<Vec<UserId>>,
    groups: Option<Vec<GroupId>>,
    permissions: Option<Vec<PermissionBody>>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum UpdateRoleError {
    RoleExists,
    RoleNotFound,
    UsersNotFound { ids: Vec<UserId> },
    GroupsNotFound { ids: Vec<GroupId> },
}

impl IntoResponse for UpdateRoleError {
    fn into_response(self) -> Response {
        match self {
            Self::RoleExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::RoleNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::UsersNotFound { .. } => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::GroupsNotFound { .. } => {
                (StatusCode::NOT_FOUND, body::Json(self)).into_response()
            }
        }
    }
}

pub async fn update_role(
    db::Conn(mut conn): db::Conn,
    initiator: Initiator,
    Path(RolePath { role_id }): Path<RolePath>,
    body::Json(json): body::Json<UpdateRole>,
) -> Result<StatusCode, Error<UpdateRoleError>> {
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Roles,
        authz::Ability::Update,
    )
    .await?;

    let mut role = Role::retrieve_id(&transaction, &role_id)
        .await?
        .ok_or(Error::Inner(UpdateRoleError::RoleNotFound))?;

    if json.name.is_some() {
        if let Some(name) = json.name {
            role.name = name;
        }

        let did_update = role.update(&transaction).await?;

        if !did_update {
            return Err(Error::Inner(UpdateRoleError::RoleExists));
        }
    }

    update_permissions(&transaction, &role, json.permissions).await?;

    match update_attached_users(&transaction, &role, json.users).await {
        Ok(_) => {}
        Err(err) => {
            return Err(match err {
                AttachedUserError::NotFound(ids) => {
                    Error::Inner(UpdateRoleError::UsersNotFound { ids })
                }
                AttachedUserError::Db(err) => Error::from(err),
            })
        }
    }

    match update_attached_groups(&transaction, &role, json.groups).await {
        Ok(_) => {}
        Err(err) => {
            return Err(match err {
                AttachedGroupError::NotFound(ids) => {
                    Error::Inner(UpdateRoleError::GroupsNotFound { ids })
                }
                AttachedGroupError::Db(err) => Error::from(err),
            })
        }
    }

    transaction.commit().await?;

    Ok(StatusCode::OK)
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum DeleteRoleError {
    RoleNotFound,
}

impl IntoResponse for DeleteRoleError {
    fn into_response(self) -> Response {
        match self {
            Self::RoleNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn delete_role(
    db::Conn(mut conn): db::Conn,
    initiator: Initiator,
    Path(RolePath { role_id }): Path<RolePath>,
) -> Result<StatusCode, Error<DeleteRoleError>> {
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        authz::Scope::Roles,
        authz::Ability::Delete,
    )
    .await?;

    let role = Role::retrieve_id(&transaction, &role_id)
        .await?
        .ok_or(Error::Inner(DeleteRoleError::RoleNotFound))?;

    let _users = transaction
        .execute("delete from user_roles where role_id = $1", &[&role.id])
        .await?;

    let _groups = transaction
        .execute("delete from group_roles where role_id = $1", &[&role.id])
        .await?;

    let _permissions = transaction
        .execute(
            "delete from authz_permissions where role_id = $1",
            &[&role.id],
        )
        .await?;

    let _role = transaction
        .execute("delete from authz_roles where id = $1", &[&role.id])
        .await?;

    transaction.commit().await?;

    Ok(StatusCode::OK)
}

fn unique_permissions(
    permissions: Vec<PermissionBody>,
) -> BTreeMap<authz::Scope, BTreeSet<authz::Ability>> {
    let mut rtn: BTreeMap<authz::Scope, BTreeSet<authz::Ability>> = BTreeMap::new();

    for perm in permissions {
        if let Some(known) = rtn.get_mut(&perm.scope) {
            known.extend(perm.abilities);
        } else {
            let value = BTreeSet::from_iter(perm.abilities);

            rtn.insert(perm.scope, value);
        }
    }

    rtn
}

async fn create_permissions(
    conn: &impl db::GenericClient,
    role: &Role,
    permissions: Vec<PermissionBody>,
) -> Result<Vec<AttachedPermission>, Error<NewRoleError>> {
    let added = Utc::now();
    let unique = unique_permissions(permissions);
    let mut rtn = Vec::new();

    if unique.is_empty() {
        return Ok(rtn);
    }

    let mut first = true;
    let mut params: db::ParamsVec<'_> = vec![&role.id, &added];
    let mut query =
        String::from("insert into authz_permissions (role_id, scope, ability, added) values ");

    for (scope, abilities) in &unique {
        if abilities.is_empty() {
            continue;
        }

        let scope_index = db::push_param(&mut params, scope);

        for ability in abilities {
            if first {
                first = false;
            } else {
                query.push_str(", ");
            }

            write!(
                &mut query,
                "($1, ${scope_index}, ${}, $2)",
                db::push_param(&mut params, ability)
            )
            .unwrap();

            rtn.push(AttachedPermission {
                scope: scope.clone(),
                ability: ability.clone(),
                added,
            });
        }
    }

    conn.execute(query.as_str(), params.as_slice()).await?;

    Ok(rtn)
}

async fn update_permissions(
    conn: &impl db::GenericClient,
    role: &Role,
    permissions: Option<Vec<PermissionBody>>,
) -> Result<Vec<AttachedPermission>, Error<UpdateRoleError>> {
    let Some(permissions) = permissions else {
        return Ok(AttachedPermission::retrieve(conn, &role.id).await?);
    };

    let stream = authz::Permission::retrieve_stream(conn, &role.id).await?;
    let mut current: HashMap<authz::Scope, HashMap<authz::Ability, authz::Permission>> =
        HashMap::new();

    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        let record = result?;

        if let Some(perms) = current.get_mut(&record.scope) {
            perms.insert(record.ability.clone(), record);
        } else {
            current.insert(
                record.scope.clone(),
                HashMap::from([(record.ability.clone(), record)]),
            );
        }
    }

    let added = Utc::now();
    let unique = unique_permissions(permissions);
    let mut rtn = Vec::new();

    if !unique.is_empty() {
        let mut first = true;
        let mut params: db::ParamsVec<'_> = vec![&role.id, &added];
        let mut query =
            String::from("insert into authz_permissions (role_id, scope, ability, added) values ");

        for (scope, abilities) in &unique {
            let scope_index = db::push_param(&mut params, scope);

            let mut known_scope = current.get_mut(scope);

            for ability in abilities {
                if first {
                    first = false;
                } else {
                    query.push_str(", ");
                }

                if let Some(abilities) = &mut known_scope {
                    abilities.remove(ability);
                }

                write!(
                    &mut query,
                    "($1, ${scope_index}, ${}, $2)",
                    db::push_param(&mut params, ability)
                )
                .unwrap();

                rtn.push(AttachedPermission {
                    scope: scope.clone(),
                    ability: ability.clone(),
                    added,
                });
            }
        }

        conn.execute(query.as_str(), params.as_slice()).await?;
    };

    if !current.is_empty() {
        let mut id_list = Vec::new();

        for (_scope, abilities) in current {
            for (_ability, record) in abilities {
                id_list.push(record.id);
            }
        }

        conn.execute(
            "delete from authz_permissions where id = any($1)",
            &[&id_list],
        )
        .await?;
    }

    Ok(rtn)
}
