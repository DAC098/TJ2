use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tj2_lib::sec::pki::PublicKey;

use crate::db;
use crate::db::ids::{UserClientId, UserPeerId};
use crate::net::{body, Error};
use crate::sec::authn::Initiator;
use crate::state;

#[derive(Debug, Serialize)]
pub struct UserKeys {
    public_key: String,
    clients: Vec<UserClient>,
    peers: Vec<UserPeer>,
}

#[derive(Debug, Serialize)]
pub struct UserClient {
    id: UserClientId,
    name: String,
    public_key: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct UserPeer {
    id: UserPeerId,
    name: String,
    public_key: String,
    addr: String,
    port: u16,
    secure: bool,
    ssc: bool,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
}

pub async fn get(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
) -> Result<body::Json<UserKeys>, Error> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db_conn().await?;

    let private_key_path = state.storage().user_dir(initiator.user.id).private_key();
    let private_key = tj2_lib::sec::pki::PrivateKey::load(&private_key_path).await?;
    let public_key = private_key.public_key();

    let (res_clients, res_peers) = tokio::join!(
        retrieve_user_clients(&conn, &initiator.user.id),
        retrieve_user_peers(&conn, &initiator.user.id),
    );

    Ok(body::Json(UserKeys {
        public_key: tj2_lib::string::to_base64(&public_key),
        clients: res_clients?,
        peers: res_peers?,
    }))
}

pub async fn retrieve_user_clients(
    conn: &impl db::GenericClient,
    users_id: &db::ids::UserId,
) -> Result<Vec<UserClient>, Error> {
    let stream = conn
        .query_raw(
            "\
        select user_clients.id, \
               user_clients.name, \
               user_clients.public_key, \
               user_clients.created, \
               user_clients.updated \
        from user_clients \
        where user_clients.users_id = $1 \
        order by user_clients.name",
            &[users_id],
        )
        .await?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let record = try_record?;

        let key: PublicKey =
            db::try_from_bytea(record.get(2)).expect("invalid public key data from db");

        rtn.push(UserClient {
            id: record.get(0),
            name: record.get(1),
            public_key: tj2_lib::string::to_base64(&key),
            created: record.get(3),
            updated: record.get(4),
        });
    }

    Ok(rtn)
}

pub async fn retrieve_user_peers(
    conn: &impl db::GenericClient,
    users_id: &db::ids::UserId,
) -> Result<Vec<UserPeer>, Error> {
    let stream = conn
        .query_raw(
            "\
        select user_peers.id, \
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
            &[users_id],
        )
        .await?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let record = try_record?;

        let key: PublicKey =
            db::try_from_bytea(record.get(2)).expect("invalid public key data from db");
        let port: u16 = db::try_from_int(record.get(4)).expect("invalid peer port data from db");

        rtn.push(UserPeer {
            id: record.get(0),
            name: record.get(1),
            public_key: tj2_lib::string::to_base64(&key),
            addr: record.get(3),
            port,
            secure: record.get(5),
            ssc: record.get(6),
            created: record.get(7),
            updated: record.get(8),
        });
    }

    Ok(rtn)
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum NewClientError {
    InvalidPublicKey,
    NameAlreadyExists,
}

impl IntoResponse for NewClientError {
    fn into_response(self) -> Response {
        match self {
            Self::InvalidPublicKey | Self::NameAlreadyExists => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum NewRecord {
    Client(NewClient),
    Peer(NewPeer),
}

#[derive(Debug, Deserialize)]
pub struct NewClient {
    name: String,
    public_key: String,
}

#[derive(Debug, Deserialize)]
pub struct NewPeer {
    name: String,
    public_key: String,
    addr: String,
    port: u16,
    secure: bool,
    ssc: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum CreatedRecord {
    Client(UserClient),
    Peer(UserPeer),
}

pub async fn post(
    state: state::SharedState,
    initiator: Initiator,
    body::Json(record): body::Json<NewRecord>,
) -> Result<(StatusCode, body::Json<CreatedRecord>), Error<NewClientError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let result = match record {
        NewRecord::Client(client) => {
            CreatedRecord::Client(create_client(&transaction, &initiator.user.id, client).await?)
        }
        NewRecord::Peer(peer) => {
            CreatedRecord::Peer(create_peer(&transaction, &initiator.user.id, peer).await?)
        }
    };

    transaction.commit().await?;

    Ok((StatusCode::CREATED, body::Json(result)))
}

pub async fn create_client(
    conn: &impl db::GenericClient,
    users_id: &db::ids::UserId,
    NewClient { name, public_key }: NewClient,
) -> Result<UserClient, Error<NewClientError>> {
    let created = Utc::now();
    let updated = None;

    let pub_key = {
        let Some(bytes) = tj2_lib::string::from_base64(&public_key) else {
            return Err(Error::Inner(NewClientError::InvalidPublicKey));
        };

        let Ok(key) = tj2_lib::sec::pki::PublicKey::from_slice(&bytes) else {
            return Err(Error::Inner(NewClientError::InvalidPublicKey));
        };

        key
    };

    let result = conn
        .query_one(
            "\
        insert into user_clients (users_id, name, public_key, created) values \
        ($1, $2, $3, $4) \
        returning id",
            &[users_id, &name, &db::ToBytea(&pub_key), &created],
        )
        .await;

    let id = match result {
        Ok(row) => row.get(0),
        Err(err) => {
            if let Some(kind) = db::ErrorKind::check(&err) {
                return match kind {
                    db::ErrorKind::Unique(constraint) => match constraint {
                        "user_clients_public_key_key" => {
                            Err(Error::Inner(NewClientError::InvalidPublicKey))
                        }
                        "user_clients_users_id_name_key" => {
                            Err(Error::Inner(NewClientError::NameAlreadyExists))
                        }
                        _ => unreachable!(),
                    },
                    _ => Err(err.into()),
                };
            } else {
                return Err(err.into());
            }
        }
    };

    Ok(UserClient {
        id,
        name,
        public_key,
        created,
        updated,
    })
}

pub async fn create_peer(
    conn: &impl db::GenericClient,
    users_id: &db::ids::UserId,
    NewPeer {
        name,
        public_key,
        addr,
        port,
        secure,
        ssc,
    }: NewPeer,
) -> Result<UserPeer, Error<NewClientError>> {
    let created = Utc::now();
    let updated = None;

    let pub_key = {
        let Some(bytes) = tj2_lib::string::from_base64(&public_key) else {
            return Err(Error::Inner(NewClientError::InvalidPublicKey));
        };

        let Ok(key) = tj2_lib::sec::pki::PublicKey::from_slice(&bytes) else {
            return Err(Error::Inner(NewClientError::InvalidPublicKey));
        };

        key
    };

    let result = conn.query_one(
        "\
        insert into user_peers (users_id, name, public_key, addr, port, secure, ssc, created) values \
        ($1, $2, $3, $4, $5, $6, $7, $8) \
        returning id",
        &[users_id, &name, &db::ToBytea(&pub_key), &addr, &db::U16toI32(&port), &secure, &ssc, &created],
    ).await;

    let id = match result {
        Ok(row) => row.get(0),
        Err(err) => {
            if let Some(kind) = db::ErrorKind::check(&err) {
                return match kind {
                    db::ErrorKind::Unique(constraint) => match constraint {
                        "user_peers_public_key_key" => {
                            Err(Error::Inner(NewClientError::InvalidPublicKey))
                        }
                        "user_peers_users_id_name_key" => {
                            Err(Error::Inner(NewClientError::NameAlreadyExists))
                        }
                        _ => unreachable!(),
                    },
                    _ => Err(err.into()),
                };
            } else {
                return Err(err.into());
            }
        }
    };

    Ok(UserPeer {
        id,
        name,
        public_key,
        addr,
        port,
        secure,
        ssc,
        created,
        updated,
    })
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum DeleteRecordError {
    IdNotFound,
}

impl IntoResponse for DeleteRecordError {
    fn into_response(self) -> Response {
        match self {
            Self::IdNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum DeleteRecord {
    Client { id: UserClientId },
    Peer { id: UserPeerId },
}

pub async fn delete(
    state: state::SharedState,
    initiator: Initiator,
    body::Json(record): body::Json<DeleteRecord>,
) -> Result<StatusCode, Error<DeleteRecordError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    match record {
        DeleteRecord::Client { id } => {
            transaction
                .execute(
                    "delete from authn_api_sessions where user_clients_id = $1",
                    &[&id],
                )
                .await?;

            let result = transaction
                .execute(
                    "delete from user_clients where users_id = $1 and id = $2",
                    &[&initiator.user.id, &id],
                )
                .await;

            match result {
                Ok(counted) => {
                    if counted != 1 {
                        return Err(Error::Inner(DeleteRecordError::IdNotFound));
                    }
                }
                Err(err) => return Err(err.into()),
            }
        }
        DeleteRecord::Peer { id } => {
            let params: db::ParamsArray<'_, 1> = [&id];

            let (synced_entries, synced_file_entries, journal_peers) = tokio::join!(
                transaction.execute(
                    "delete from synced_entries where user_peers_id = $1",
                    &params
                ),
                transaction.execute(
                    "delete from synced_file_entries where user_peers_id = $1",
                    &params
                ),
                transaction.execute(
                    "delete from journal_peers where user_peers_id = $1",
                    &params
                ),
            );

            synced_entries?;
            synced_file_entries?;
            journal_peers?;

            let result = transaction
                .execute(
                    "delete from user_peers where users_id = $1 and id = $2",
                    &[&initiator.user.id, &id],
                )
                .await;

            match result {
                Ok(counted) => {
                    if counted != 1 {
                        return Err(Error::Inner(DeleteRecordError::IdNotFound));
                    }
                }
                Err(err) => return Err(err.into()),
            }
        }
    }

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
