use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};

use crate::db;
use crate::db::ids::{UserId, UserUid, GroupId, GroupUid};

#[derive(Debug)]
pub struct User {
    pub id: UserId,
    pub uid: UserUid,
    pub username: String,
    pub password: String,
    pub version: i64,
}

impl User {
    pub async fn retrieve_username(conn: &impl db::GenericClient, username: &str) -> Result<Option<Self>, db::PgError> {
        conn.query_opt(
            "\
            select id, \
                   uid, \
                   username, \
                   password, \
                   version \
            from users \
            where username = $1",
            &[&username]
        )
            .await
            .map(|maybe| maybe.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                username: row.get(2),
                password: row.get(3),
                version: row.get(4),
            }))
    }

    pub async fn retrieve_id(conn: &impl db::GenericClient, id: UserId) -> Result<Option<Self>, db::PgError> {
        conn.query_opt(
            "\
            select id, \
                   uid, \
                   username, \
                   password, \
                   version \
            from users \
            where id = $1",
            &[&id]
        )
            .await
            .map(|maybe| maybe.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                username: row.get(2),
                password: row.get(3),
                version: row.get(4),
            }))
    }

    pub async fn create(conn: &impl db::GenericClient, username: &str, hash: &str, version: i64) -> Result<Option<Self>, db::PgError> {
        let uid = UserUid::gen();

        let result = conn.query_opt(
            "\
            insert into users (uid, username, password, version) \
            values ($1, $2, $3, $4) \
            on conflict on constraint users_username_key do nothing \
            returning id",
            &[&uid, &username, &hash, &version]
        ).await?;

        match result {
            Some(row) => Ok(Some(Self {
                id: row.get(0),
                uid,
                username: username.to_owned(),
                password: hash.to_owned(),
                version
            })),
            None => Ok(None)
        }
    }
}

#[derive(Debug)]
pub struct Group {
    pub id: GroupId,
    pub uid: GroupUid,
    pub name: String,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl Group {
    pub async fn retrieve_id(conn: &impl db::GenericClient, groups_id: GroupId) -> Result<Option<Self>, db::PgError> {
        conn.query_opt(
            "\
            select id, \
                   uid, \
                   name, \
                   created, \
                   updated \
            from groups \
            where id = $1",
            &[&groups_id]
        )
            .await
            .map(|maybe| maybe.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                name: row.get(2),
                created: row.get(3),
                updated: row.get(4),
            }))
    }

    pub async fn create(conn: &impl db::GenericClient, name: &str) -> Result<Option<Self>, db::PgError> {
        let uid = GroupUid::gen();
        let created = Utc::now();

        let result = conn.query_opt(
            "\
            insert into groups (uid, name, created) values \
            ($1, $2, $3) \
            on conflict on constraint groups_name_key do nothing \
            returning id",
            &[&uid, &name, &created]
        ).await?;

        match result {
            Some(row) => Ok(Some(Self {
                id: row.get(0),
                uid,
                name: name.to_owned(),
                created,
                updated: None
            })),
            None => Ok(None),
        }
    }
}

pub struct GroupUser {
    pub groups_id: GroupId,
    pub users_id: UserId,
    pub added: DateTime<Utc>,
}

impl GroupUser {
    pub async fn retrieve_groups_id_stream(
        conn: &impl db::GenericClient,
        groups_id: &GroupId
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [groups_id];

        let stream = conn.query_raw(
            "\
            select group_users.groups_id, \
                   group_users.users_id, \
                   group_users.added \
            from group_users \
            where group_users.groups_id = $1",
            params
        ).await?;

        Ok(stream.map(|result| result.map(|row| Self {
            groups_id: row.get(0),
            users_id: row.get(1),
            added: row.get(2),
        })))
    }

    pub async fn retrieve_users_id_stream(
        conn: &impl db::GenericClient,
        users_id: &UserId,
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [users_id];

        let stream = conn.query_raw(
            "\
            select group_users.groups_id, \
                   group_users.users_id, \
                   group_users.added \
            from group_users \
            where group_users.users_id = $1",
            params
        ).await?;

        Ok(stream.map(|result| result.map(|row| Self {
            groups_id: row.get(0),
            users_id: row.get(1),
            added: row.get(2),
        })))
    }
}

pub async fn assign_user_group(
    conn: &impl db::GenericClient,
    users_id: UserId,
    groups_id: GroupId
) -> Result<(), db::PgError> {
    let added = Utc::now();

    conn.execute(
        "\
        insert into group_users (users_id, groups_id, added) values \
        ($1, $2, $3)",
        &[&users_id, &groups_id, &added]
    ).await?;

    Ok(())
}

