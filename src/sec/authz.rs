use sqlx::Type;

use crate::db;
use crate::db::ids::{RefId, UserId, RoleId, RoleUid};

#[derive(Debug, Type)]
#[sqlx(rename_all = "lowercase")]
pub enum Ability {
    Read,
    Write,
}

#[derive(Debug, Type)]
#[sqlx(rename_all = "lowercase")]
pub enum Scope {
    Users,
    Journals,
    Entries,
}

#[derive(Debug)]
pub struct Permission {
    pub role_id: RoleId,
    pub scope: Scope,
    pub ability: Ability,
    pub ref_id: i64,
}

#[derive(Debug)]
pub struct Role {
    pub id: RoleId,
    pub uid: RoleUid,
    pub name: String,
}

pub async fn has_permission(
    conn: &mut db::DbConn,
    users_id: UserId,
    scope: Scope,
    ability: Ability,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "\
        select authz_permissions.role_id \
        from authz_permissions \
            join authz_roles on \
                authz_permissions.role_id = authz_roles.id \
            left join group_roles on \
                authz_roles.id = group_roles.role_id \
            left join groups on \
                group_roles.group_id = groups.id \
            left join group_users on \
                groups.id = group_users.group_id \
            left join user_roles on \
                authz_roles.id = user_roles.role_id \
        where (user_roles.user_id = $1 or group_users.user_id = $1) and \
            authz_permissions.scope = $2 and \
            authz_permissions.ability = $3"
    )
        .bind(users_id)
        .bind(scope)
        .bind(ability)
        .execute(&mut *conn)
        .await?
        .rows_affected();

    Ok(result > 0)
}

pub async fn has_permission_ref<'a, T>(
    conn: &mut db::DbConn,
    users_id: UserId,
    scope: Scope,
    ability: Ability,
    ref_id: T
) -> Result<bool, sqlx::Error>
where
    T: RefId + sqlx::Encode<'a, db::Db> + 'a
{
    let result = sqlx::query(
        "\
        select authz_permissions.role_id \
        from authz_permissions \
            join authz_roles on \
                authz_permissions.role_id = authz_roles.id \
            left join group_roles on \
                authz_roles.id = group_roles.role_id \
            left join groups on \
                group_roles.group_id = groups.id \
            left join group_users on \
                groups.id = group_users.group_id \
            left join user_roles on \
                authz_roles.id = user_roles.role_id \
        where (user_roles.user_id = ?1 or group_users.user_id = ?1) and \
            authz_permissions.scope = ?2 and \
            authz_permissions.ability = ?3 and \
            authz_permissions.ref_id = ?4"
    )
        .bind(users_id)
        .bind(scope)
        .bind(ability)
        .bind(ref_id)
        .execute(&mut *conn)
        .await?
        .rows_affected();

    Ok(result > 0)
}
