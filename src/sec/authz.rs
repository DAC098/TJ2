use std::collections::HashMap;
use std::fmt::{Write, Display, Formatter, Result as FmtResult};
use std::str::FromStr;

use bytes::BytesMut;
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use postgres_types as pg_types;
use serde::{Serialize, Deserialize};

use crate::db;
use crate::db::ids::{GroupId, UserId, RoleId, RoleUid, PermissionId};
use crate::error::{self, Context, BoxDynError};
use crate::user::User;
use crate::user::group::Group;

#[derive(Debug, thiserror::Error)]
#[error("the provided string is not a valid Ability")]
pub struct InvalidAbility;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ability {
    Create,
    Read,
    Update,
    Delete,
}

impl Ability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Ability::Create => "create",
            Ability::Read => "read",
            Ability::Update => "update",
            Ability::Delete => "delete",
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
            "create" => Ok(Ability::Create),
            "read" => Ok(Ability::Read),
            "update" => Ok(Ability::Update),
            "delete" => Ok(Ability::Delete),
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
            Scope::Users => "users",
            Scope::Groups => "groups",
            Scope::Journals => "journals",
            Scope::Entries => "entries",
            Scope::Roles => "roles",
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
            "users" => Ok(Scope::Users),
            "groups" => Ok(Scope::Groups),
            "journals" => Ok(Scope::Journals),
            "entries" => Ok(Scope::Entries),
            "roles" => Ok(Scope::Roles),
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
    pub added: DateTime<Utc>,
}

impl Permission {
    pub async fn retrieve_stream(
        conn: &impl db::GenericClient,
        role_id: &RoleId
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [role_id];

        let stream = conn.query_raw(
            "\
            select authz_permissions.id, \
                   authz_permissions.role_id, \
                   authz_permissions.scope, \
                   authz_permissions.ability, \
                   authz_permissions.ref_id, \
                   authz_permissions.added \
            from authz_permissions \
            where authz_permissions.role_id = $1 \
            order by authz_permissions.scope, \
                     authz_permissions.ability",
            params
        )
            .await?;

        Ok(stream.map(|result| result.map(|row| Self {
            id: row.get(0),
            role_id: row.get(1),
            scope: row.get(2),
            ability: row.get(3),
            ref_id: row.get(4),
            added: row.get(5),
        })))
    }
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
    pub async fn retrieve_id(conn: &impl db::GenericClient, role_id: &RoleId) -> Result<Option<Self>, db::PgError> {
        conn.query_opt(
            "\
            select authz_roles.id, \
                   authz_roles.uid, \
                   authz_roles.name, \
                   authz_roles.created, \
                   authz_roles.updated \
            from authz_roles \
            where authz_roles.id = $1",
            &[role_id]
        )
            .await
            .map(|result| result.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                name: row.get(2),
                created: row.get(3),
                updated: row.get(4),
            }))
    }

    pub async fn create(conn: &impl db::GenericClient, name: &str) -> Result<Option<Self>, db::PgError> {
        let uid = RoleUid::gen();
        let created = Utc::now();

        let result = conn.query_one(
            "\
            insert into authz_roles (uid, name, created) values \
            ($1, $2, $3) \
            returning id",
            &[&uid, &name, &created]
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

    pub async fn update(&mut self, conn: &impl db::GenericClient) -> Result<bool, db::PgError> {
        self.updated = Some(Utc::now());

        let result = conn.execute(
            "\
            update authz_roles \
            set name = $2, \
                updated = $3 \
            where id = $1",
            &[&self.id, &self.name, &self.updated]
        ).await;

        match result {
            Ok(count) => Ok(count == 1),
            Err(err) => if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::Unique(constraint) => if constraint == "authz_roles_name_key" {
                        Ok(false)
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
    let added = Utc::now();
    let mut top_first = true;
    let mut params: db::ParamsVec<'_> = vec![&id, &added];
    let mut query = String::from(
        "insert into authz_permissions (role_id, scope, ability, added) values "
    );

    for (scope, abilities) in list {
        let mut first = true;

        if top_first {
            top_first = false;
        } else {
            query.push_str(", ");
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
                db::push_param(&mut params, ability),
            ).unwrap();
        }
    }

    tracing::debug!("query: \"{query}\"");

    conn.execute(query.as_str(), &params)
        .await?;

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum RefId<'a> {
    User(&'a UserId),
    Group(&'a GroupId),
}

impl<'a> From<&'a User> for RefId<'a> {
    fn from(user: &'a User) -> Self {
        Self::User(&user.id)
    }
}

impl<'a> From<&'a Group> for RefId<'a> {
    fn from(group: &'a Group) -> Self {
        Self::Group(&group.id)
    }
}

#[derive(Debug, Serialize)]
pub struct AttachedRole {
    pub role_id: RoleId,
    pub name: String,
    pub added: DateTime<Utc>,
}

impl AttachedRole {
    pub async fn retrieve_stream<'a, I>(
        conn: &impl db::GenericClient,
        id: I
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError>
    where
        I: Into<RefId<'a>>
    {
        let stream = match id.into() {
            RefId::User(users_id) => {
                let params: db::ParamsArray<'_, 1> = [users_id];

                conn.query_raw(
                    "\
                    select user_roles.role_id, \
                           authz_roles.name, \
                           user_roles.added \
                    from user_roles \
                        left join authz_roles on \
                            user_roles.role_id = authz_roles.id \
                    where user_roles.users_id = $1",
                    params
                ).await?
            }
            RefId::Group(groups_id) => {
                let params: db::ParamsArray<'_, 1> = [groups_id];

                conn.query_raw(
                    "\
                    select group_roles.role_id, \
                           authz_roles.name, \
                           group_roles.added \
                    from group_roles \
                        left join authz_roles on \
                            group_roles.role_id = authz_roles.id \
                    where group_roles.groups_id = $1",
                    params
                ).await?
            }
        };

        Ok(stream.map(|result| result.map(|row| Self {
            role_id: row.get(0),
            name: row.get(1),
            added: row.get(2),
        })))
    }

    pub async fn retrieve<'a, I>(
        conn: &impl db::GenericClient,
        id: I
    ) -> Result<Vec<Self>, error::Error>
    where
        I: Into<RefId<'a>>
    {
        let stream = Self::retrieve_stream(conn, id)
            .await
            .context("failed to retrieve attached roles")?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(result) = stream.next().await {
            let record = result.context("failed to retrieve attached role record")?;

            rtn.push(record);
        }

        Ok(rtn)
    }
}

pub async fn create_attached_roles<'a, I>(
    conn: &impl db::GenericClient,
    id: I,
    roles: Vec<RoleId>,
) -> Result<(Vec<AttachedRole>, Vec<RoleId>), error::Error>
where
    I: Into<RefId<'a>>
{
    if roles.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let added = Utc::now();
    let (mut requested, roles, _common) = db::ids::unique_ids::<RoleId, ()>(roles, None);

    let stream = match id.into() {
        RefId::User(users_id) => {
            let params: db::ParamsArray<'_, 3> = [users_id, &added, &roles];

            conn.query_raw(
                "\
                with tmp_insert as ( \
                    insert into user_roles (role_id, users_id, added) \
                    select authz_roles.id as role_id, \
                           $1::bigint as users_id, \
                           $2::timestamp with time zone as added \
                    from authz_roles \
                    where authz_roles.id = any($3) \
                    returning * \
                ) \
                select tmp_insert.role_id, \
                       authz_roles.name, \
                       tmp_insert.added \
                from tmp_insert \
                    left join authz_roles on \
                        tmp_insert.role_id = authz_roles.id",
                params
            )
                .await
                .context("failed to add roles to user")?
        }
        RefId::Group(groups_id) => {
            let params: db::ParamsArray<'_, 3> = [groups_id, &added, &roles];

            conn.query_raw(
                "\
                with tmp_insert as ( \
                    insert into group_roles (role_id, groups_id, added) \
                    select authz_roles.id as role_id, \
                           $1::bigint as groups_id, \
                           $2::timestamp with time zone as added \
                    from authz_roles \
                    where authz_roles.id = any($3) \
                    returning * \
                ) \
                select tmp_insert.role_id, \
                       authz_roles.name, \
                       tmp_insert.added \
                from tmp_insert \
                    left join authz_roles on \
                        tmp_insert.role_id = authz_roles.id",
                params
            )
                .await
                .context("failed to add roles to group")?
        }
    };

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(result) = stream.next().await {
        let record = result.context("failed to retrieve added role")?;
        let role_id = record.get(0);

        if !requested.remove(&role_id) {
            tracing::warn!("a role was added that was not requested");
        }

        rtn.push(AttachedRole {
            role_id,
            name: record.get(1),
            added: record.get(2),
        });
    }

    Ok((rtn, Vec::new()))
}

pub async fn update_attached_roles<'a, I>(
    conn: &impl db::GenericClient,
    id: I,
    roles: Option<Vec<RoleId>>,
) -> Result<(Vec<AttachedRole>, Vec<RoleId>), error::Error>
where
    I: Into<RefId<'a>>,
{
    let id = id.into();

    let Some(roles) = roles else {
        return Ok((AttachedRole::retrieve(conn, id).await?, Vec::new()));
    };

    let added = Utc::now();
    let mut current: HashMap<RoleId, AttachedRole> = HashMap::new();
    let stream = AttachedRole::retrieve_stream(conn, id)
        .await
        .context("failed to retrieve currently attached roles")?;

    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        let record = result.context("failed to retrieve current attached group")?;

        current.insert(record.role_id, record);
    }

    let (mut requested, roles, common) = db::ids::unique_ids(roles, Some(&mut current));

    let mut rtn = Vec::from_iter(common.into_values());

    if !requested.is_empty() {
        let stream = match id {
            RefId::User(users_id) => {
                let params: db::ParamsArray<'_, 3> = [users_id, &added, &roles];

                conn.query_raw(
                    "\
                    with tmp_insert as ( \
                        insert into user_roles (role_id, users_id, added) \
                        select authz_roles.id, \
                               $1::bigint as users_id, \
                               $2::timestamp with time zone as added \
                        from authz_roles \
                        where authz_roles.id = any($3) \
                        on conflict on constraint user_roles_pkey do nothing \
                        returning * \
                    ) \
                    select tmp_insert.role_id, \
                           authz_roles.name, \
                           tmp_insert.added \
                    from tmp_insert \
                        left join authz_roles on \
                            tmp_insert.role_id = authz_roles.id",
                    params
                )
                    .await
                    .context("failed to add roles to user")?
            }
            RefId::Group(groups_id) => {
                let params: db::ParamsArray<'_, 3> = [groups_id, &added, &roles];

                conn.query_raw(
                    "\
                    with tmp_insert as ( \
                        insert into group_roles (role_id, groups_id, added) \
                        select authz_roles.id as role_id, \
                               $1::bigint as groups_id, \
                               $2::timestamp with time zone as added \
                        from authz_roles \
                        where authz_roles.id = any($3) \
                        on conflict on constraint group_roles_pkey do nothing \
                        returning * \
                    ) \
                    select tmp_insert.role_id, \
                           authz_roles.name, \
                           tmp_insert.added \
                    from tmp_insert \
                        left join authz_roles on \
                            tmp_insert.role_id = authz_roles.id",
                    params
                )
                    .await
                    .context("failed to add roles to group")?
            }
        };

        futures::pin_mut!(stream);

        while let Some(result) = stream.next().await {
            let record = result.context("failed to retrieve added role")?;
            let role_id = record.get(0);

            if !requested.remove(&role_id) {
                tracing::warn!("a role was added that was not requested");
            }

            rtn.push(AttachedRole {
                role_id,
                name: record.get(1),
                added: record.get(2),
            });
        }
    }

    if !current.is_empty() {
        let to_delete = Vec::from_iter(current.into_keys());

        match id {
            RefId::User(users_id) => {
                conn.execute(
                    "delete from user_roles where users_id = $1 and role_id = any($2)",
                    &[users_id, &to_delete]
                )
                    .await
                    .context("failed to delete from user roles")?;
            }
            RefId::Group(groups_id) => {
                conn.execute(
                    "delete from group_roles where groups_id = $1 and role_id = any($2)",
                    &[groups_id, &to_delete]
                )
                    .await
                    .context("failed to delete from user roles")?;
            }
        }
    }

    Ok((rtn, Vec::from_iter(requested)))
}
