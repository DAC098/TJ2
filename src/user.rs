use sqlx::Row;

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
    pub async fn retrieve_username(conn: &mut db::DbConn, username: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query(
            "\
            select id, \
                   uid, \
                   username, \
                   password, \
                   version \
            from users \
            where username = ?1"
        ).bind(username)
            .fetch_optional(&mut *conn)
            .await
            .map(|maybe| maybe.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                username: row.get(2),
                password: row.get(3),
                version: row.get(4),
            }))
    }

    pub async fn retrieve_id(conn: &mut db::DbConn, id: UserId) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query(
            "\
            select id, \
                   uid, \
                   username, \
                   password, \
                   version \
            from users \
            where id = ?1"
        ).bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map(|maybe| maybe.map(|row| Self {
                id: row.get(0),
                uid: row.get(1),
                username: row.get(2),
                password: row.get(3),
                version: row.get(4),
            }))
    }

    pub async fn create(conn: &mut db::DbConn, username: &str, hash: &str, version: i64) -> Result<Self, sqlx::Error> {
        let uid = UserUid::gen();

        sqlx::query(
            "\
            insert into users (uid, username, password, version) \
            values (?1, ?2, ?3, ?4) \
            returning id"
        )
            .bind(&uid)
            .bind(username)
            .bind(hash)
            .bind(version)
            .fetch_one(&mut *conn)
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
    pub async fn create(conn: &mut db::DbConn, name: &str) -> Result<Self, sqlx::Error> {
        let uid = GroupUid::gen();

        sqlx::query(
            "\
            insert into groups (uid, name) values \
            (?1, ?2) \
            returning id"
        )
            .bind(&uid)
            .bind(name)
            .fetch_one(&mut *conn)
            .await
            .map(|row| Self {
                id: row.get(0),
                uid,
                name: name.to_owned(),
            })
    }
}

pub async fn assign_user_group(
    conn: &mut db::DbConn,
    users_id: UserId,
    groups_id: GroupId
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "\
        insert into group_users (users_id, groups_id) values \
        (?1, ?2)"
    )
        .bind(users_id)
        .bind(groups_id)
        .execute(&mut *conn)
        .await?;

    Ok(())
}
