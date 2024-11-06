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

    pub async fn create(conn: &impl db::GenericClient, username: &str, hash: &str, version: i64) -> Result<Self, db::PgError> {
        let uid = UserUid::gen();

        conn.query_one(
            "\
            insert into users (uid, username, password, version) \
            values ($1, $2, $3, $4) \
            returning id",
            &[&uid, &username, &hash, &version]
        )
            .await
            .map(|row| Self {
                id: row.get(0),
                uid,
                username: username.to_owned(),
                password: hash.to_owned(),
                version
            })
    }
}

#[derive(Debug)]
pub struct Group {
    pub id: GroupId,
    pub uid: GroupUid,
    pub name: String
}

impl Group {
    pub async fn create(conn: &impl db::GenericClient, name: &str) -> Result<Self, db::PgError> {
        let uid = GroupUid::gen();

        conn.query_one(
            "\
            insert into groups (uid, name) values \
            ($1, $2) \
            returning id",
            &[&name, &uid]
        )
            .await
            .map(|row| Self {
                id: row.get(0),
                uid,
                name: name.to_owned(),
            })
    }
}

pub async fn assign_user_group(
    conn: &impl db::GenericClient,
    users_id: UserId,
    groups_id: GroupId
) -> Result<(), db::PgError> {
    conn.execute(
        "\
        insert into group_users (users_id, groups_id) values \
        ($1, $2)",
        &[&users_id, &groups_id]
    )
        .await?;

    Ok(())
}

