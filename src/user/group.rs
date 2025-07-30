use std::collections::HashMap;

use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use serde::Serialize;

use crate::db;
use crate::db::ids::{GroupId, GroupUid, RoleId, UserId};
use crate::sec::authz::Role;

use super::User;

#[derive(Debug)]
pub struct Group {
    pub id: GroupId,
    pub uid: GroupUid,
    pub name: String,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

pub enum RetrieveGroup<'a> {
    Id(&'a GroupId),
}

impl<'a> From<&'a GroupId> for RetrieveGroup<'a> {
    fn from(given: &'a GroupId) -> Self {
        Self::Id(given)
    }
}

impl Group {
    pub async fn retrieve<'a, T>(
        conn: &impl db::GenericClient,
        given: T,
    ) -> Result<Option<Self>, db::PgError>
    where
        T: Into<RetrieveGroup<'a>>,
    {
        match given.into() {
            RetrieveGroup::Id(groups_id) => {
                conn.query_opt(
                    "\
                select groups.id, \
                       groups.uid, \
                       groups.name, \
                       groups.created, \
                       groups.updated \
                from groups \
                where groups.id = $1",
                    &[&groups_id],
                )
                .await
            }
        }
        .map(|result| {
            result.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                name: row.get(2),
                created: row.get(3),
                updated: row.get(4),
            })
        })
    }

    pub async fn retrieve_id(
        conn: &impl db::GenericClient,
        groups_id: GroupId,
    ) -> Result<Option<Self>, db::PgError> {
        Self::retrieve(conn, &groups_id).await
    }

    pub async fn create(
        conn: &impl db::GenericClient,
        name: &str,
    ) -> Result<Option<Self>, db::PgError> {
        let uid = GroupUid::gen();
        let created = Utc::now();

        let result = conn
            .query_opt(
                "\
            insert into groups (uid, name, created) values \
            ($1, $2, $3) \
            on conflict on constraint groups_name_key do nothing \
            returning id",
                &[&uid, &name, &created],
            )
            .await?;

        match result {
            Some(row) => Ok(Some(Self {
                id: row.get(0),
                uid,
                name: name.to_owned(),
                created,
                updated: None,
            })),
            None => Ok(None),
        }
    }

    pub async fn update(&mut self, conn: &impl db::GenericClient) -> Result<bool, db::PgError> {
        self.updated = Some(Utc::now());

        let result = conn
            .execute(
                "\
            update groups \
            set name = $2, \
                updated = $3
            where id = $1",
                &[&self.id, &self.name, &self.updated],
            )
            .await;

        match result {
            Ok(count) => Ok(count == 1),
            Err(err) => {
                if let Some(kind) = db::ErrorKind::check(&err) {
                    match kind {
                        db::ErrorKind::Unique(constraint) => {
                            if constraint == "groups_name_key" {
                                Ok(false)
                            } else {
                                Err(err)
                            }
                        }
                        _ => Err(err),
                    }
                } else {
                    Err(err)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GroupRefId<'a> {
    User(&'a UserId),
    Role(&'a RoleId),
}

impl<'a> From<&'a User> for GroupRefId<'a> {
    fn from(user: &'a User) -> Self {
        Self::User(&user.id)
    }
}

impl<'a> From<&'a Role> for GroupRefId<'a> {
    fn from(role: &'a Role) -> Self {
        Self::Role(&role.id)
    }
}

#[derive(Debug, Serialize)]
pub struct AttachedGroup {
    pub groups_id: GroupId,
    pub name: String,
    pub added: DateTime<Utc>,
}

impl AttachedGroup {
    pub async fn retrieve_stream<'a, I>(
        conn: &impl db::GenericClient,
        id: I,
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError>
    where
        I: Into<GroupRefId<'a>>,
    {
        let stream = match id.into() {
            GroupRefId::User(users_id) => {
                let params: db::ParamsArray<'_, 1> = [users_id];

                conn.query_raw(
                    "\
                    select group_users.groups_id, \
                           groups.name, \
                           group_users.added \
                    from group_users \
                        left join groups on \
                            group_users.groups_id = groups.id \
                    where group_users.users_id = $1",
                    params,
                )
                .await?
            }
            GroupRefId::Role(role_id) => {
                let params: db::ParamsArray<'_, 1> = [role_id];

                conn.query_raw(
                    "\
                    select group_roles.groups_id, \
                           groups.name,
                           group_roles.added \
                    from group_roles \
                        left join groups on \
                            group_roles.groups_id = groups.id \
                    where group_roles.role_id = $1",
                    params,
                )
                .await?
            }
        };

        Ok(stream.map(|result| {
            result.map(|row| Self {
                groups_id: row.get(0),
                name: row.get(1),
                added: row.get(2),
            })
        }))
    }

    pub async fn retrieve<'a, I>(
        conn: &impl db::GenericClient,
        id: I,
    ) -> Result<Vec<Self>, db::PgError>
    where
        I: Into<GroupRefId<'a>>,
    {
        let stream = Self::retrieve_stream(conn, id).await?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(result) = stream.next().await {
            rtn.push(result?);
        }

        Ok(rtn)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AttachedGroupError {
    #[error("the following group ids where not found")]
    NotFound(Vec<GroupId>),

    #[error(transparent)]
    Db(#[from] db::PgError),
}

pub async fn create_attached_groups<'a, I>(
    conn: &impl db::GenericClient,
    id: I,
    groups: Vec<GroupId>,
) -> Result<Vec<AttachedGroup>, AttachedGroupError>
where
    I: Into<GroupRefId<'a>>,
{
    if groups.is_empty() {
        return Ok(Vec::new());
    }

    let added = Utc::now();
    let (mut requested, groups, _common) = db::ids::unique_ids::<GroupId, ()>(groups, None);

    let stream = match id.into() {
        GroupRefId::User(users_id) => {
            let params: db::ParamsArray<'_, 3> = [users_id, &added, &groups];

            conn.query_raw(
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
                params,
            )
            .await?
        }
        GroupRefId::Role(role_id) => {
            let params: db::ParamsArray<'_, 3> = [role_id, &added, &groups];

            conn.query_raw(
                "\
                with tmp_insert as ( \
                    insert into group_roles (groups_id, role_id, added) \
                    select groups.id, \
                           $1::bigint as role_id, \
                           $2::timestamp with time zone as added \
                    from groups \
                    where groups.id = any($3) \
                    returning * \
                ) \
                select tmp_insert.groups_id,
                       groups.name,
                       tmp_insert.added \
                from tmp_insert \
                    left join groups on \
                        tmp_insert.groups_id = groups.id",
                params,
            )
            .await?
        }
    };

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(result) = stream.next().await {
        let record = result?;
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

    let not_found = Vec::from_iter(requested);

    if !not_found.is_empty() {
        Err(AttachedGroupError::NotFound(not_found))
    } else {
        Ok(rtn)
    }
}

pub async fn update_attached_groups<'a, I>(
    conn: &impl db::GenericClient,
    id: I,
    groups: Option<Vec<GroupId>>,
) -> Result<Vec<AttachedGroup>, AttachedGroupError>
where
    I: Into<GroupRefId<'a>>,
{
    let id = id.into();

    let Some(groups) = groups else {
        return Ok(AttachedGroup::retrieve(conn, id).await?);
    };

    let added = Utc::now();
    let mut current: HashMap<GroupId, AttachedGroup> = HashMap::new();
    let stream = AttachedGroup::retrieve_stream(conn, id).await?;

    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        let record = result?;

        current.insert(record.groups_id, record);
    }

    let (mut requested, groups, common) = db::ids::unique_ids(groups, Some(&mut current));

    let mut rtn = Vec::from_iter(common.into_values());

    if !requested.is_empty() {
        let stream = match id {
            GroupRefId::User(users_id) => {
                let params: db::ParamsArray<'_, 3> = [users_id, &added, &groups];

                conn.query_raw(
                    "\
                    with tmp_insert as ( \
                        insert into group_users (groups_id, users_id, added) \
                        select groups.id as groups_id, \
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
                    params,
                )
                .await?
            }
            GroupRefId::Role(role_id) => {
                let params: db::ParamsArray<'_, 3> = [role_id, &added, &groups];

                conn.query_raw(
                    "\
                    with tmp_insert as ( \
                        insert into group_roles (groups_id, role_id, added) \
                        select groups.id as groups_id, \
                               $1::bigint as role_id, \
                               $2::timestamp with time zone as added \
                        from groups \
                        where groups.id = any($3) \
                        on conflict on constraint group_roles_pkey do nothing \
                        returning * \
                    ) \
                    select tmp_insert.groups_id, \
                           groups.name, \
                           tmp_insert.added \
                    from tmp_insert \
                        left join groups on \
                            tmp_insert.groups_id = groups.id",
                    params,
                )
                .await?
            }
        };

        futures::pin_mut!(stream);

        while let Some(result) = stream.next().await {
            let record = result?;
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
    }

    if !current.is_empty() {
        let to_delete = Vec::from_iter(current.into_keys());

        match id {
            GroupRefId::User(users_id) => {
                conn.execute(
                    "delete from group_users where users_id = $1 and groups_id = any($2)",
                    &[users_id, &to_delete],
                )
                .await?;
            }
            GroupRefId::Role(role_id) => {
                conn.execute(
                    "delete from group_roles where role_id = $1 and groups_id = any($2)",
                    &[role_id, &to_delete],
                )
                .await?;
            }
        }
    }

    let not_found = Vec::from_iter(requested);

    if !not_found.is_empty() {
        Err(AttachedGroupError::NotFound(not_found))
    } else {
        Ok(rtn)
    }
}

pub async fn assign_user_group(
    conn: &impl db::GenericClient,
    users_id: UserId,
    groups_id: GroupId,
) -> Result<(), db::PgError> {
    let added = Utc::now();

    conn.execute(
        "\
        insert into group_users (users_id, groups_id, added) values \
        ($1, $2, $3)",
        &[&users_id, &groups_id, &added],
    )
    .await?;

    Ok(())
}
