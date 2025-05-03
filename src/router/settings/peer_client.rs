use axum::extract::Query;
use axum::http::{StatusCode, HeaderMap};
use axum::response::{Response, IntoResponse};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Serialize, Deserialize};
use tj2_lib::sec::pki::PublicKey;

use crate::db;
use crate::error::{self, Context};
use crate::router::{body, macros};
use crate::state::{self, Security};
use crate::sec::authn::Initiator;

#[derive(Debug, Serialize)]
pub struct UserKeys {
    public_key: String,
    clients: Vec<UserClient>,
    peers: Vec<UserPeer>,
}

#[derive(Debug, Serialize)]
pub struct UserClient {
    name: String,
    public_key: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct UserPeer {
    name: String,
    public_key: String,
    peer_addr: String,
    peer_port: u16,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
}

pub async fn get(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let conn = state.db_conn().await?;

    let private_key_path = state.storage()
        .user_dir(initiator.user.id)
        .private_key();
    let private_key = tj2_lib::sec::pki::PrivateKey::load(&private_key_path)
        .await
        .context("failed to load private key")?;
    let public_key = private_key.public_key();

    let (res_clients, res_peers) = tokio::join!(
        retrieve_user_clients(&conn, &initiator.user.id),
        retrieve_user_peers(&conn, &initiator.user.id),
    );

    Ok(body::Json(UserKeys {
        public_key: tj2_lib::string::to_hex_str(&public_key),
        clients: res_clients?,
        peers: res_peers?,
    }).into_response())
}

pub async fn retrieve_user_clients(
    conn: &impl db::GenericClient,
    users_id: &db::ids::UserId,
) -> Result<Vec<UserClient>, error::Error> {
    let stream = conn.query_raw(
        "\
        select user_client_keys.name, \
               user_client_keys.public_key, \
               user_client_keys.created, \
               user_client_keys.updated \
        from user_client_keys \
        where user_client_keys.users_id = $1",
        &[users_id]
    ).await.context("failed to retrieve client keys")?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let record = try_record.context("failed to retrieve record")?;

        let key: PublicKey = db::try_from_bytea(record.get(1))
            .expect("invalid public key data from db");

        rtn.push(UserClient {
            name: record.get(0),
            public_key: tj2_lib::string::to_hex_str(&key),
            created: record.get(2),
            updated: record.get(3),
        });
    }

    Ok(rtn)
}

pub async fn retrieve_user_peers(
    conn: &impl db::GenericClient,
    users_id: &db::ids::UserId
) -> Result<Vec<UserPeer>, error::Error> {
    let stream = conn.query_raw(
        "\
        select user_peer_keys.name, \
               user_peer_keys.public_key, \
               user_peer_keys.peer_addr, \
               user_peer_keys.peer_port, \
               user_peer_keys.created, \
               user_peer_kyes.updated \
        from user_peer_keys \
        where user_peer_keys.users_id = $1",
        &[users_id]
    ).await.context("failed to retrieve peer keys")?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let record = try_record.context("failed to retrieve record")?;

        let key: PublicKey = db::try_from_bytea(record.get(1))
            .expect("invalid public key data from db");
        let peer_port: u16 = db::try_from_int(record.get(3))
            .expect("invalid peer port data from db");

        rtn.push(UserPeer {
            name: record.get(0),
            public_key: tj2_lib::string::to_hex_str(&key),
            peer_addr: record.get(2),
            peer_port,
            created: record.get(4),
            updated: record.get(5),
        });
    }

    Ok(rtn)
}

#[derive(Debug, thiserror::Error, Serialize)]
pub enum NewClientError {
    #[serde(skip)]
    #[error(transparent)]
    Db(#[from] db::PgError),

    #[serde(skip)]
    #[error(transparent)]
    DbPool(#[from] db::PoolError),

    #[serde(skip)]
    #[error(transparent)]
    Error(#[from] error::Error),
}

pub async fn post(
    state: state::SharedState,
    initiator: Initiator,
) -> Result<(), NewClientError> {

    Ok(())
}
