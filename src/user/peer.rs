use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use tj2_lib::sec::pki::PublicKey;

use crate::db;
use crate::db::ids::{JournalId, UserId, UserPeerId};

pub struct UserPeer {
    pub id: UserPeerId,
    pub users_id: UserId,
    pub name: String,
    pub public_key: PublicKey,
    pub addr: String,
    pub port: u16,
    pub secure: bool,
    pub ssc: bool,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

pub enum RetrieveOne<'a> {
    Id(&'a UserPeerId)
}

impl<'a> From<&'a UserPeerId> for RetrieveOne<'a> {
    fn from(given: &'a UserPeerId) -> Self {
        Self::Id(given)
    }
}

pub enum RetrieveMany<'a> {
    UserId(&'a UserId),
    JournalId(&'a JournalId),
}

impl<'a> From<&'a UserId> for RetrieveMany<'a> {
    fn from(given: &'a UserId) -> Self {
        Self::UserId(given)
    }
}

impl<'a> From<&'a JournalId> for RetrieveMany<'a> {
    fn from(given: &'a JournalId) -> Self {
        Self::JournalId(given)
    }
}

impl UserPeer {
    pub async fn retrieve<'a, T>(
        conn: &impl db::GenericClient,
        given: T
    ) -> Result<Option<Self>, db::PgError>
    where
        T: Into<RetrieveOne<'a>>,
    {
        let result = match given.into() {
            RetrieveOne::Id(id) => conn.query_opt(
                "\
                select user_peers.id, \
                       user_peers.users_id, \
                       user_peers.name, \
                       user_peers.public_key, \
                       user_peers.addr, \
                       user_peers.port, \
                       user_peers.secure, \
                       user_peers.ssc, \
                       user_peers.created, \
                       user_peers.updated \
                from user_peers \
                where user_peers.id = $1",
                &[id]
            ).await?
        };

        Ok(result.map(|record| {
            let port: u16 = db::try_from_int(record.get(5))
                .expect("invalid peer port from db");
            let public_key: PublicKey = db::try_from_bytea(record.get(3))
                .expect("invalid public key data from db");

            Self {
                id: record.get(0),
                users_id: record.get(1),
                name: record.get(2),
                public_key,
                addr: record.get(4),
                port,
                secure: record.get(6),
                ssc: record.get(7),
                created: record.get(8),
                updated: record.get(9),
            }
        }))
    }

    pub async fn retrieve_many<'a, T>(
        conn: &impl db::GenericClient,
        given: T
    ) -> Result<impl Stream<Item = Result<Self, db::PgError>>, db::PgError>
    where
        T: Into<RetrieveMany<'a>>
    {
        Ok(match given.into() {
            RetrieveMany::UserId(users_id) => {
                let params: db::ParamsArray<'a, 1> = [users_id];

                conn.query_raw(
                    "\
                    select user_peers.id, \
                           user_peers.users_id, \
                           user_peers.name, \
                           user_peers.public_key, \
                           user_peers.addr, \
                           user_peers.port, \
                           user_peers.secure, \
                           user_peers.ssc, \
                           user_peers.created, \
                           user_peers.updated \
                    from user_peers \
                    where user_peers.users_id = $1 \
                    order by user_peers.name",
                    params
                ).await?
            },
            RetrieveMany::JournalId(journals_id) => {
                let params: db::ParamsArray<'a, 1> = [journals_id];

                conn.query_raw(
                    "\
                    select user_peers.id, \
                           user_peers.users_id, \
                           user_peers.name, \
                           user_peers.public_key, \
                           user_peers.addr, \
                           user_peers.port, \
                           user_peers.secure, \
                           user_peers.ssc, \
                           user_peers.created, \
                           user_peers.updated \
                    from user_peers \
                        right join journal_peers on \
                            user_peers.id = journal_peers.user_peers_id \
                    where journal_peers.journals_id = $1 \
                    order by user_peers.name",
                    params
                ).await?
            }
        }.map(|maybe| maybe.map(|record| {
            let port: u16 = db::try_from_int(record.get(5))
                .expect("invalid peer port from db");
            let public_key: PublicKey = db::try_from_bytea(record.get(3))
                .expect("invalid public key data from db");

            Self {
                id: record.get(0),
                users_id: record.get(1),
                name: record.get(2),
                public_key,
                addr: record.get(4),
                port,
                secure: record.get(6),
                ssc: record.get(7),
                created: record.get(8),
                updated: record.get(9),
            }
        })))
    }
}
