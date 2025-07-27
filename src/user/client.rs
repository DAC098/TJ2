use chrono::{DateTime, Utc};
use tj2_lib::sec::pki::PublicKey;

use crate::db;
use crate::db::ids::{UserClientId, UserId};

#[derive(Debug)]
pub struct UserClient {
    pub id: UserClientId,
    pub users_id: UserId,
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub public_key: PublicKey,
    #[allow(dead_code)]
    pub created: DateTime<Utc>,
    #[allow(dead_code)]
    pub updated: Option<DateTime<Utc>>,
}

pub enum RetrieveOne<'a> {
    PublicKey(&'a PublicKey),
}

impl<'a> From<&'a PublicKey> for RetrieveOne<'a> {
    fn from(given: &'a PublicKey) -> Self {
        Self::PublicKey(given)
    }
}

impl UserClient {
    pub async fn retrieve<'a, T>(
        conn: &impl db::GenericClient,
        given: T,
    ) -> Result<Option<Self>, db::PgError>
    where
        T: Into<RetrieveOne<'a>>,
    {
        Ok(match given.into() {
            RetrieveOne::PublicKey(key) => {
                conn.query_opt(
                    "\
                select user_clients.id, \
                       user_clients.users_id, \
                       user_clients.name, \
                       user_clients.public_key, \
                       user_clients.created, \
                       user_clients.updated \
                from user_clients \
                where user_clients.public_key = $1",
                    &[&db::ToBytea(key)],
                )
                .await?
            }
        }
        .map(|record| Self {
            id: record.get(0),
            users_id: record.get(1),
            name: record.get(2),
            public_key: db::try_from_bytea(record.get(3))
                .expect("invalid public key stored in database"),
            created: record.get(4),
            updated: record.get(5),
        }))
    }
}
