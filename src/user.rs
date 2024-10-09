use sqlx::Row;

use crate::db;
use crate::db::ids::{UserId, UserUid};

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
}
