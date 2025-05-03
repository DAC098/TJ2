use chrono::{DateTime, Utc};
use tj2_lib::sec::pki::PublicKey;

use crate::db;
use crate::db::ids::UserId;

#[derive(Debug)]
pub struct UserClientKey {
    pub users_id: UserId,
    pub name: String,
    pub public_key: PublicKey,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

pub enum RetrieveQuery<'a> {
    UserId(&'a UserId),
    PublicKey(&'a PublicKey),
}

impl<'a> From<&'a UserId> for RetrieveQuery<'a> {
    fn from(id: &'a UserId) -> Self {
        Self::UserId(id)
    }
}

impl<'a> From<&'a PublicKey> for RetrieveQuery<'a> {
    fn from(key: &'a PublicKey) -> Self {
        Self::PublicKey(key)
    }
}

impl UserClientKey {
    pub async fn retrieve<'a, T>(conn: &impl db::GenericClient, given: T) -> Result<Option<Self>, db::PgError>
    where
        T: Into<RetrieveQuery<'a>>
    {
        let result = match given.into() {
            RetrieveQuery::UserId(users_id) => conn.query_opt(
                "\
                select user_client_keys.users_id, \
                       user_client_keys.name, \
                       user_client_keys.public_key, \
                       user_client_keys.created, \
                       user_client_keys.updated \
                from user_client_keys \
                where user_client_keys.users_id = $1",
                &[users_id]
            ).await?,
            RetrieveQuery::PublicKey(public_key) => conn.query_opt(
                "\
                select user_client_keys.users_id, \
                       user_client_keys.name, \
                       user_client_keys.public_key, \
                       user_client_keys.created, \
                       user_client_keys.updated \
                from user_client_keys \
                where user_client_keys.public_key = $1",
                &[&db::ToBytea(public_key)]
            ).await?,
        };

        Ok(result.map(|record| Self {
            users_id: record.get(0),
            name: record.get(1),
            public_key: db::try_from_bytea(record.get(2))
                .expect("invalid public key from database"),
            created: record.get(3),
            updated: record.get(4),
        }))
    }
}
