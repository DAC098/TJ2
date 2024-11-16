use std::fmt::{Write, Display, Formatter, Result as FmtResult};
use std::str::FromStr;

use bytes::BytesMut;
use chrono::{DateTime, Utc};
use postgres_types as pg_types;

use crate::db;
use crate::db::ids::{GroupId, UserId, RoleId, RoleUid, PermissionId};
use crate::error::BoxDynError;

#[derive(Debug, thiserror::Error)]
#[error("the provided string is not a valid Ability")]
pub struct InvalidAbility;

#[derive(Debug, Clone)]
pub enum Ability {
    Create,
    Read,
    Update,
    Delete,
}

impl Ability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Ability::Create => "Create",
            Ability::Read => "Read",
            Ability::Update => "Update",
            Ability::Delete => "Delete",
        }
    }
}

impl Display for Ability {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.as_str())
    }
}

impl FromStr for Ability {
    type Err = InvalidAbility;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Create" => Ok(Ability::Create),
            "Read" => Ok(Ability::Read),
            "Update" => Ok(Ability::Update),
            "Delete" => Ok(Ability::Delete),
            _ => Err(InvalidAbility)
        }
    }
}

impl<'a> pg_types::FromSql<'a> for Ability {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let v = <&str as pg_types::FromSql>::from_sql(ty, raw)?;

        Ok(Self::from_str(v)?)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&str as pg_types::FromSql>::accepts(ty)
    }
}

impl pg_types::ToSql for Ability {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        self.as_str()
            .to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&str as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

#[derive(Debug, thiserror::Error)]
#[error("the provided string is not a valid scope")]
pub struct InvalidScope;

#[derive(Debug, Clone)]
pub enum Scope {
    Users,
    Groups,
    Journals,
    Entries,
    Roles,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Users => "Users",
            Scope::Groups => "Groups",
            Scope::Journals => "Journals",
            Scope::Entries => "Entries",
            Scope::Roles => "Roles",
        }
    }
}

impl Display for Scope {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.as_str())
    }
}

impl FromStr for Scope {
    type Err = InvalidScope;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Users" => Ok(Scope::Users),
            "Groups" => Ok(Scope::Groups),
            "Journals" => Ok(Scope::Journals),
            "Entries" => Ok(Scope::Entries),
            "Roles" => Ok(Scope::Roles),
            _ => Err(InvalidScope),
        }
    }
}

impl<'a> pg_types::FromSql<'a> for Scope {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let v = <&str as pg_types::FromSql>::from_sql(ty, raw)?;

        Ok(Self::from_str(v)?)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&str as pg_types::FromSql>::accepts(ty)
    }
}

impl pg_types::ToSql for Scope {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        self.as_str()
            .to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&str as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

#[derive(Debug)]
pub struct Permission {
    pub id: PermissionId,
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
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl Role {
    pub async fn create(conn: &impl db::GenericClient, name: &str) -> Result<Option<Self>, db::PgError> {
        let uid = RoleUid::gen();
        let created = Utc::now();

        let result = conn.query_one(
            "\
            insert into authz_roles (uid, name, created) values \
            ($1, $2, $3) \
            returning id",
            &[&uid, &name]
        ).await;

        match result {
            Ok(row) => Ok(Some(Self {
                id: row.get(0),
                uid,
                name: name.to_owned(),
                created,
                updated: None
            })),
            Err(err) => if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::Unique(constraint) => if constraint == "authz_roles_name_key" {
                        Ok(None)
                    } else {
                        Err(err)
                    }
                    _ => Err(err)
                }
            } else {
                Err(err)
            }
        }
    }

    pub async fn assign_user(&self, conn: &impl db::GenericClient, users_id: UserId) -> Result<(), db::PgError> {
        assign_user_role(conn, self.id, users_id).await
    }

    pub async fn assign_group(&self, conn: &impl db::GenericClient, groups_id: GroupId) -> Result<(), db::PgError> {
        assign_group_role(conn, self.id, groups_id).await
    }

    pub async fn assign_permissions<'a, I>(&self, conn: &impl db::GenericClient, list: I) -> Result<(), db::PgError>
    where
        I: IntoIterator<Item = &'a (Scope, Vec<Ability>)>
    {
        create_permissions(conn, self.id, list).await
    }
}

pub async fn has_permission(
    conn: &impl db::GenericClient,
    users_id: UserId,
    scope: Scope,
    ability: Ability
) -> Result<bool, db::PgError> {
    let result = conn.execute(
        "\
        select authz_permissions.role_id \
        from authz_permissions \
            join authz_roles on \
                authz_permissions.role_id = authz_roles.id \
            left join group_roles on \
                authz_roles.id = group_roles.role_id \
            left join groups on \
                group_roles.groups_id = groups.id \
            left join group_users on \
                groups.id = group_users.groups_id \
            left join user_roles on \
                authz_roles.id = user_roles.role_id \
        where (user_roles.users_id = $1 or group_users.users_id = $1) and \
            authz_permissions.scope = $2 and \
            authz_permissions.ability = $3 and \
            authz_permissions.ref_id is null",
        &[&users_id, &scope.as_str(), &ability.as_str()]
    ).await?;

    Ok(result > 0)
}

pub async fn has_permission_ref<'a, T>(
    conn: &impl db::GenericClient,
    users_id: UserId,
    scope: Scope,
    ability: Ability,
    ref_id: T,
) -> Result<bool, db::PgError>
where
    T: AsRef<i64>
{
    let id = ref_id.as_ref();

    let result = conn.execute(
        "\
        select authz_permissions.role_id \
        from authz_permissions \
            join authz_roles on \
                authz_permissions.role_id = authz_roles.id \
            left join group_roles on \
                authz_roles.id = group_roles.role_id \
            left join groups on \
                group_roles.groups_id = groups.id \
            left join group_users on \
                groups.id = group_users.groups_id \
            left join user_roles on \
                authz_roles.id = user_roles.role_id \
        where (user_roles.users_id = $1 or group_users.users_id = $1) and \
            authz_permissions.scope = $2 and \
            authz_permissions.ability = $3 and \
            authz_permissions.ref_id = $4",
        &[&users_id, &scope, &ability, id]
    ).await?;

    Ok(result > 0)
}

pub async fn assign_user_role(
    conn: &impl db::GenericClient,
    role_id: RoleId,
    users_id: UserId,
) -> Result<(), db::PgError> {
    let added = Utc::now();

    conn.execute(
        "insert into user_roles (users_id, role_id, added) values ($1, $2, $3)",
        &[&users_id, &role_id, &added]
    ).await?;

    Ok(())
}

pub async fn assign_group_role(
    conn: &impl db::GenericClient,
    role_id: RoleId,
    groups_id: GroupId,
) -> Result<(), db::PgError> {
    let added = Utc::now();

    conn.execute(
        "insert into group_roles (groups_id, role_id, added) values ($1, $2, $3)",
        &[&groups_id, &role_id, &added]
    ).await?;

    Ok(())
}

pub async fn create_permissions<'a, I>(
    conn: &impl db::GenericClient,
    id: RoleId,
    list: I
) -> Result<(), db::PgError>
where
    I: IntoIterator<Item = &'a (Scope, Vec<Ability>)>
{
    let mut top_first = true;
    let mut params: db::ParamsVec<'_> = vec![&id];
    let mut query = String::from(
        "insert into authz_permissions (role_id, scope, ability) values "
    );

    for (scope, abilities) in list {
        let mut first = true;

        if top_first {
            top_first = false;
        } else {
            query.push_str(", ");
        }

        for ability in abilities {
            if first {
                first = false;
            } else {
                query.push_str(", ");
            }

            write!(
                &mut query,
                "($1, ${}, ${})",
                db::push_param(&mut params, scope),
                db::push_param(&mut params, ability),
            ).unwrap();
        }
    }

    tracing::debug!("query: \"{query}\"");

    conn.execute(query.as_str(), &params)
        .await?;

    Ok(())
}
