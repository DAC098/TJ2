use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use serde::Serialize;

use crate::db;
use crate::db::ids::{GroupId, RoleId, UserId, UserUid};
use crate::sec;
use crate::sec::authz::Role;

pub mod client;
pub mod group;
pub mod invite;
pub mod peer;

use group::Group;

#[derive(Debug)]
pub struct User {
    pub id: UserId,
    pub uid: UserUid,
    pub username: String,
    pub password: String,
    pub version: i64,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub enum PasswordKind {
    Hashed(String, i64),
    Unhashed(String),
}

#[derive(Debug, Clone)]
pub struct UserBuilder {
    username: String,
    password: PasswordKind,
    uid: Option<UserUid>,
}

#[derive(Debug, thiserror::Error)]
pub enum UserBuilderError {
    #[error("username already exists")]
    UsernameExists,

    #[error("uid already exists")]
    UidExists,

    #[error(transparent)]
    Argon(#[from] sec::password::HashError),

    #[error(transparent)]
    Db(#[from] db::PgError),
}

pub enum RetrieveUserQuery<'a> {
    Username(&'a str),
    Id(&'a UserId),
    Uid(&'a UserUid),
}

impl<'a> From<&'a String> for RetrieveUserQuery<'a> {
    fn from(given: &'a String) -> Self {
        Self::Username(given.as_str())
    }
}

impl<'a> From<&'a str> for RetrieveUserQuery<'a> {
    fn from(given: &'a str) -> Self {
        Self::Username(given)
    }
}

impl<'a> From<&'a UserId> for RetrieveUserQuery<'a> {
    fn from(given: &'a UserId) -> Self {
        Self::Id(given)
    }
}

impl<'a> From<&'a UserUid> for RetrieveUserQuery<'a> {
    fn from(given: &'a UserUid) -> Self {
        Self::Uid(given)
    }
}

impl User {
    pub async fn retrieve<'a, T>(
        conn: &impl db::GenericClient,
        given: T,
    ) -> Result<Option<Self>, db::PgError>
    where
        T: Into<RetrieveUserQuery<'a>>,
    {
        match given.into() {
            RetrieveUserQuery::Username(username) => {
                conn.query_opt(
                    "\
                select id, \
                       uid, \
                       username, \
                       password, \
                       version, \
                       created, \
                       updated \
                from users \
                where username = $1",
                    &[&username],
                )
                .await
            }
            RetrieveUserQuery::Id(id) => {
                conn.query_opt(
                    "\
                select id, \
                       uid, \
                       username, \
                       password, \
                       version, \
                       created, \
                       updated \
                from users \
                where id = $1",
                    &[id],
                )
                .await
            }
            RetrieveUserQuery::Uid(uid) => {
                conn.query_opt(
                    "\
                select id, \
                       uid, \
                       username, \
                       password, \
                       version, \
                       created, \
                       updated \
                from users \
                where uid = $1",
                    &[uid],
                )
                .await
            }
        }
        .map(|maybe| {
            maybe.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                username: row.get(2),
                password: row.get(3),
                version: row.get(4),
                created: row.get(5),
                updated: row.get(6),
            })
        })
    }

    pub async fn retrieve_username(
        conn: &impl db::GenericClient,
        username: &str,
    ) -> Result<Option<Self>, db::PgError> {
        Self::retrieve(conn, RetrieveUserQuery::Username(username)).await
    }

    pub async fn retrieve_id(
        conn: &impl db::GenericClient,
        id: UserId,
    ) -> Result<Option<Self>, db::PgError> {
        Self::retrieve(conn, RetrieveUserQuery::Id(&id)).await
    }

    pub async fn create(
        conn: &impl db::GenericClient,
        username: &str,
        hash: &str,
        version: i64,
    ) -> Result<Self, UserBuilderError> {
        let builder = UserBuilder::new_hash(username.to_owned(), hash.to_owned(), version);

        builder.build(conn).await
    }

    pub async fn update(&mut self, conn: &impl db::GenericClient) -> Result<bool, db::PgError> {
        self.updated = Some(Utc::now());

        let result = conn
            .execute(
                "\
            update users \
            set username = $2, \
                password = $3, \
                version = $4, \
                updated = $5 \
            where id = $1",
                &[
                    &self.id,
                    &self.username,
                    &self.password,
                    &self.version,
                    &self.updated,
                ],
            )
            .await;

        match result {
            Ok(result) => Ok(result == 1),
            Err(err) => {
                if let Some(kind) = db::ErrorKind::check(&err) {
                    match kind {
                        db::ErrorKind::Unique(constraint) => {
                            if constraint == "users_username_key" {
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

impl UserBuilder {
    /// user builder with a pre generated argon hash
    pub fn new_hash(username: String, hash: String, version: i64) -> Self {
        Self {
            username,
            password: PasswordKind::Hashed(hash, version),
            uid: None,
        }
    }

    /// user builder that will generate a argon hash from the given password
    pub fn new_password(username: String, password: String) -> Self {
        Self {
            username,
            password: PasswordKind::Unhashed(password),
            uid: None,
        }
    }

    pub async fn build(self, conn: &impl db::GenericClient) -> Result<User, UserBuilderError> {
        let username = self.username;
        let (password, version) = match self.password {
            PasswordKind::Hashed(p, v) => (p, v),
            PasswordKind::Unhashed(p) => (sec::password::create(p)?, 0),
        };
        let uid = self.uid.unwrap_or(UserUid::gen());
        let created = Utc::now();

        let result = conn
            .query_one(
                "\
            insert into users (uid, username, password, version, created) \
            values ($1, $2, $3, $4, $5) \
            returning id",
                &[&uid, &username, &password, &version, &created],
            )
            .await;

        match result {
            Ok(row) => Ok(User {
                id: row.get(0),
                uid,
                username,
                password,
                version,
                created,
                updated: None,
            }),
            Err(err) => {
                if let Some(kind) = db::ErrorKind::check(&err) {
                    match kind {
                        db::ErrorKind::Unique(constraint) => match constraint {
                            "users_username_key" => Err(UserBuilderError::UsernameExists),
                            "users_uid_key" => Err(UserBuilderError::UidExists),
                            _ => Err(err.into()),
                        },
                        _ => Err(err.into()),
                    }
                } else {
                    Err(err.into())
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UserRefId<'a> {
    Group(&'a GroupId),
    Role(&'a RoleId),
}

impl<'a> From<&'a Group> for UserRefId<'a> {
    fn from(group: &'a Group) -> Self {
        Self::Group(&group.id)
    }
}

impl<'a> From<&'a Role> for UserRefId<'a> {
    fn from(role: &'a Role) -> Self {
        Self::Role(&role.id)
    }
}

#[derive(Debug, Serialize)]
pub struct AttachedUser {
    pub users_id: UserId,
    pub username: String,
    pub added: DateTime<Utc>,
}

impl AttachedUser {
    pub async fn retrieve_stream<'a, I>(
        conn: &impl db::GenericClient,
        id: I,
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError>
    where
        I: Into<UserRefId<'a>>,
    {
        let stream = match id.into() {
            UserRefId::Group(groups_id) => {
                let params: db::ParamsArray<'_, 1> = [groups_id];

                conn.query_raw(
                    "\
                    select group_users.users_id, \
                           users.username, \
                           group_users.added \
                    from group_users \
                        left join users on \
                            group_users.users_id = users.id \
                    where group_users.groups_id = $1",
                    params,
                )
                .await?
            }
            UserRefId::Role(role_id) => {
                let params: db::ParamsArray<'_, 1> = [role_id];

                conn.query_raw(
                    "\
                    select user_roles.users_id, \
                           users.username, \
                           user_roles.added \
                    from user_roles \
                        left join users on \
                            user_roles.users_id = users.id \
                    where user_roles.role_id = $1",
                    params,
                )
                .await?
            }
        };

        Ok(stream.map(|result| {
            result.map(|row| Self {
                users_id: row.get(0),
                username: row.get(1),
                added: row.get(2),
            })
        }))
    }

    pub async fn retrieve<'a, I>(
        conn: &impl db::GenericClient,
        id: I,
    ) -> Result<Vec<Self>, db::PgError>
    where
        I: Into<UserRefId<'a>>,
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
pub enum AttachedUserError {
    #[error("the following user ids where not found")]
    NotFound(Vec<UserId>),

    #[error(transparent)]
    Db(#[from] db::PgError),
}

pub async fn create_attached_users<'a, I>(
    conn: &impl db::GenericClient,
    id: I,
    users: Vec<UserId>,
) -> Result<Vec<AttachedUser>, AttachedUserError>
where
    I: Into<UserRefId<'a>>,
{
    if users.is_empty() {
        return Ok(Vec::new());
    }

    let added = Utc::now();
    let (mut requested, users, _diff) = db::ids::unique_ids::<UserId, ()>(users, None);

    let stream = match id.into() {
        UserRefId::Group(groups_id) => {
            let params: db::ParamsArray<'_, 3> = [groups_id, &added, &users];

            conn.query_raw(
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
                params,
            )
            .await?
        }
        UserRefId::Role(role_id) => {
            let params: db::ParamsArray<'_, 3> = [role_id, &added, &users];

            conn.query_raw(
                "\
                with tmp_insert as ( \
                    insert into user_roles (users_id, role_id, added) \
                    select users.id as users_id, \
                           $1::bigint as role_id, \
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
                params,
            )
            .await?
        }
    };

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(result) = stream.next().await {
        let record = result?;
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

    let not_found = Vec::from_iter(requested);

    if !not_found.is_empty() {
        Err(AttachedUserError::NotFound(not_found))
    } else {
        Ok(rtn)
    }
}

pub async fn update_attached_users<'a, I>(
    conn: &impl db::GenericClient,
    id: I,
    users: Option<Vec<UserId>>,
) -> Result<Vec<AttachedUser>, AttachedUserError>
where
    I: Into<UserRefId<'a>>,
{
    let id = id.into();

    let Some(users) = users else {
        return Ok(AttachedUser::retrieve(conn, id).await?);
    };

    let added = Utc::now();
    let mut current: HashMap<UserId, AttachedUser> = HashMap::new();
    let stream = AttachedUser::retrieve_stream(conn, id).await?;

    futures::pin_mut!(stream);

    while let Some(result) = stream.next().await {
        let record = result?;

        current.insert(record.users_id, record);
    }

    tracing::debug!("current groups: {current:#?}");

    let (mut requested, users, common) = db::ids::unique_ids(users, Some(&mut current));

    let mut rtn = Vec::from_iter(common.into_values());

    if !requested.is_empty() {
        tracing::debug!("users: {users:#?}");

        let stream = match id {
            UserRefId::Group(groups_id) => {
                let params: db::ParamsArray<'_, 3> = [groups_id, &added, &users];

                conn.query_raw(
                    "\
                    with tmp_insert as ( \
                        insert into group_users (users_id, groups_id, added) \
                        select users.id as users_id, \
                               $1::bigint as groups_id, \
                               $2::timestamp with time zone as added \
                        from users \
                        where users.id = any($3) \
                        on conflict on constraint group_users_pkey do nothing \
                        returning * \
                    ) \
                    select tmp_insert.users_id, \
                           users.username, \
                           tmp_insert.added \
                    from tmp_insert \
                        left join users on \
                            tmp_insert.users_id = users.id",
                    params,
                )
                .await?
            }
            UserRefId::Role(role_id) => {
                let params: db::ParamsArray<'_, 3> = [role_id, &added, &users];

                conn.query_raw(
                    "\
                    with tmp_insert as ( \
                        insert into user_roles (users_id, role_id, added) \
                        select users.id as users_id, \
                               $1::bigint as role_id, \
                               $2::timestamp with time zone as added \
                        from users \
                        where users.id = any($3) \
                        on conflict on constraint user_roles_pkey do nothing \
                        returning * \
                    ) \
                    select tmp_insert.users_id, \
                           users.username, \
                           tmp_insert.added \
                    from tmp_insert \
                        left join users on \
                            tmp_insert.users_id = users.id",
                    params,
                )
                .await?
            }
        };

        futures::pin_mut!(stream);

        while let Some(result) = stream.next().await {
            let record = result?;
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
    }

    if !current.is_empty() {
        let to_delete = Vec::from_iter(current.into_keys());

        match id {
            UserRefId::Group(groups_id) => {
                conn.execute(
                    "delete from group_users where groups_id = $1 and users_id = any($2)",
                    &[groups_id, &to_delete],
                )
                .await?;
            }
            UserRefId::Role(role_id) => {
                conn.execute(
                    "delete from user_roles where role_id = $1 and users_id = any($2)",
                    &[role_id, &to_delete],
                )
                .await?;
            }
        }
    }

    let not_found = Vec::from_iter(requested);

    if !not_found.is_empty() {
        Err(AttachedUserError::NotFound(not_found))
    } else {
        Ok(rtn)
    }
}

#[derive(Debug)]
pub struct UserDir {
    root: PathBuf,
}

impl UserDir {
    pub fn new<P>(root: P, users_id: UserId) -> Self
    where
        P: AsRef<Path>,
    {
        let path = format!("users/{users_id}");

        Self {
            root: root.as_ref().join(path),
        }
    }

    pub async fn create(&self) -> Result<(), std::io::Error> {
        tokio::fs::create_dir(&self.root).await?;

        Ok(())
    }

    pub fn private_key(&self) -> PathBuf {
        self.root.join("private.key")
    }
}
