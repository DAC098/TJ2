use sqlx::{QueryBuilder, Row, Type, Execute};

use crate::db;
use crate::db::ids::{GroupId, UserId, RoleId, RoleUid};

#[derive(Debug, Clone, Type)]
#[sqlx(rename_all = "lowercase")]
pub enum Ability {
    Create,
    Read,
    Update,
    Delete,
}

#[derive(Debug, Clone, Type)]
#[sqlx(rename_all = "lowercase")]
pub enum Scope {
    Users,
    Journals,
    Entries,
    Roles,
}

#[derive(Debug)]
pub struct Permission {
    pub role_id: RoleId,
    pub scope: Scope,
    pub ability: Ability,
    pub ref_id: Option<i64>,
}

#[derive(Debug)]
pub struct Role {
    pub id: RoleId,
    pub uid: RoleUid,
    pub name: String,
}

impl Role {
    pub async fn create(conn: &mut db::DbConn, name: &str) -> Result<Self, sqlx::Error> {
        let uid = RoleUid::gen();

        sqlx::query(
            "\
            insert into authz_roles (uid, name) values \
            (?1, ?2) \
            returning id"
        )
            .bind(&uid)
            .bind(name)
            .fetch_one(conn)
            .await
            .map(|row| Self {
                id: row.get(0),
                uid,
                name: name.to_owned()
            })
    }
}

pub async fn has_permission(
    conn: &mut db::DbConn,
    users_id: UserId,
    scope: Scope,
    ability: Ability,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "\
        select count(authz_permissions.role_id) \
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
        .fetch_one(&mut *conn)
        .await?;

    let counted: i64 = result.get(0);

    Ok(counted > 0)
}

pub async fn has_permission_ref<'a, T>(
    conn: &mut db::DbConn,
    users_id: UserId,
    scope: Scope,
    ability: Ability,
    ref_id: T
) -> Result<bool, sqlx::Error>
where
    T: AsRef<i64>
{
    let id = ref_id.as_ref();

    let result = sqlx::query(
        "\
        select count(authz_permissions.role_id) \
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
        where (user_roles.users_id = ?1 or group_users.users_id = ?1) and \
            authz_permissions.scope = ?2 and \
            authz_permissions.ability = ?3 and \
            (authz_permissions.ref_id = ?4 or authz_permissions.ref_id is null)"
    )
        .bind(users_id)
        .bind(scope)
        .bind(ability)
        .bind(id)
        .fetch_one(&mut *conn)
        .await?;

    let counted: i64 = result.get(0);

    Ok(counted > 0)
}

pub async fn assign_user_role(
    conn: &mut db::DbConn,
    users_id: UserId,
    role_id: RoleId,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "insert into user_roles (users_id, role_id) values (?1, ?2)"
    )
        .bind(users_id)
        .bind(role_id)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

pub async fn assign_group_role(
    conn: &mut db::DbConn,
    group_id: GroupId,
    role_id: RoleId,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "insert into group_roles (groups_id, role_id) values (?1, ?2)"
    )
        .bind(group_id)
        .bind(role_id)
        .execute(&mut *conn)
        .await?;

    Ok(())
}

pub async fn create_permissions<I>(
    conn: &mut db::DbConn,
    id: RoleId,
    list: I,
) -> Result<(), sqlx::Error>
where
    I: IntoIterator<Item = (Scope, Vec<Ability>)>
{
    let mut query_builder: QueryBuilder<db::Db> = sqlx::QueryBuilder::new(
        "insert into authz_permissions (role_id, scope, ability) values "
    );

    let mut top_first = true;

    for (scope, abilities) in list {
        let mut first = true;

        if top_first {
            top_first = false;
        } else {
            query_builder.push(", ");
        }

        for ability in abilities {
            if first {
                query_builder.push("(");
                first = false;
            } else {
                query_builder.push(", (");
            }

            let mut separated = query_builder.separated(", ");
            separated.push_bind(id);
            separated.push_bind(scope.clone());
            separated.push_bind(ability);
            separated.push_unseparated(")");
        }
    }

    let query = query_builder.build();
    let sql = query.sql();

    tracing::debug!("permissions sql: {sql}");

    query.execute(&mut *conn)
        .await?;

    Ok(())
}
