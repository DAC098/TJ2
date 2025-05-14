use std::net::{Ipv4Addr, Ipv6Addr};

use axum::http::StatusCode;
use crypto_box::ChaChaBox;
use serde::{Serialize, Deserialize};
use tj2_lib::sec::pki::{PrivateKey, PublicKey, PrivateKeyError};
use url::Url;

use crate::api;
use crate::db;
use crate::db::ids::RemoteServerId;
use crate::sec::pki;
use crate::sec::authn::ApiSession;
use crate::state::Storage;
use crate::user::peer::UserPeer;

pub mod journal;

#[derive(Debug, Serialize)]
pub struct RemoteServer {
    pub id: RemoteServerId,
    pub addr: String,
    pub port: u16,
    pub secure: bool,
    pub public_key: PublicKey,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PeerAddr {
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
    Domain(String),
}

#[derive(Debug)]
pub enum RetrieveQuery<'a> {
    Id(&'a RemoteServerId),
    Key(&'a PublicKey)
}

impl RemoteServer {
    pub fn get_port(given: i32) -> u16 {
        match given.try_into() {
            Ok(valid) => valid,
            Err(_) => panic!("invalid port value from remote server database record. value: {given}"),
        }
    }

    pub async fn retrieve<'a, T>(
        conn: &impl db::GenericClient,
        query: T
    ) -> Result<Option<Self>, db::PgError>
    where
        T: Into<RetrieveQuery<'a>>
    {
        match query.into() {
            RetrieveQuery::Id(id) => conn.query_opt(
                "\
                select remote_servers.id, \
                       remote_servers.addr, \
                       remote_servers.port, \
                       remote_servers.secure, \
                       remote_servers.public_key \
                from remote_servers \
                where remote_servers.id = $1",
                &[id]
            ).await,
            RetrieveQuery::Key(key) => conn.query_opt(
                "\
                select remote_servers.id, \
                       remote_servers.addr, \
                       remote_servers.port, \
                       remote_servers.secure, \
                       remote_servers.public_key \
                from remote_servers \
                where remote_servers.public_key = $1",
                &[&db::ToBytea(key)]
            ).await
        }
            .map(|result| result.map(|row| {
                let public_key = PublicKey::from_slice(row.get(4))
                    .expect("invalid public key size stored in database");

                Self {
                    id: row.get(0),
                    addr: row.get(1),
                    port: Self::get_port(row.get(2)),
                    secure: row.get(3),
                    public_key,
                }
            }))
    }

    pub fn id(&self) -> &RemoteServerId {
        &self.id
    }
}

impl PeerAddr {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Domain(domain) => {
                let to_check = format!("http://{domain}/");

                Url::parse(&to_check).is_ok()
            },
            _ => true
        }
    }

    pub fn to_valid_string(&self) -> Option<String> {
        match self {
            Self::Ipv4(ip) => Some(format!("{ip}")),
            Self::Ipv6(ip) => Some(format!("[{ip}]")),
            Self::Domain(domain) => {
                let to_check = format!("http://{domain}/");

                if Url::parse(&to_check).is_ok() {
                    Some(domain.clone())
                } else {
                    None
                }
            }
        }
    }
}

impl<'a> From<&'a RemoteServerId> for RetrieveQuery<'a> {
    fn from(id: &'a RemoteServerId) -> Self {
        Self::Id(id)
    }
}

impl<'a> From<&'a PublicKey> for RetrieveQuery<'a> {
    fn from(key: &'a PublicKey) -> Self {
        Self::Key(key)
    }
}

static CLIENT_USER_AGENT: &str = "tj2-sync-client/0.0.1";

pub struct RemoteClient {
    remote: RemoteServer,
    origin: String,
    client: reqwest::Client,
}

impl RemoteClient {
    pub fn build(remote: RemoteServer) -> Result<Self, reqwest::Error> {
        let origin = if remote.secure {
            format!("https://{}:{}", remote.addr, remote.port)
        } else {
            format!("http://{}:{}", remote.addr, remote.port)
        };
        let client = reqwest::Client::builder()
            .user_agent(CLIENT_USER_AGENT)
            .build()?;

        Ok(Self {
            remote,
            origin,
            client,
        })
    }

    pub fn remote(&self) -> &RemoteServer {
        &self.remote
    }

    pub fn post<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>
    {
        let mut url = self.origin.clone();
        url.push_str(path.as_ref());

        self.client.post(url)
    }

    pub fn put<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>
    {
        let mut url = self.origin.clone();
        url.push_str(path.as_ref());

        self.client.put(url)
    }
}

fn origin_url<P>(origin: &str, path: P) -> String
where
    P: AsRef<str>
{
    let mut url = origin.to_owned();
    url.push_str(path.as_ref());

    url
}

#[derive(Debug)]
pub struct PeerClient {
    peer: UserPeer,
    origin: String,
    client: reqwest::Client,
}

#[derive(Debug)]
pub struct PeerClientBuilder {
    peer: UserPeer,
    origin: String,
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("failed to receive authn challenge from peer")]
    FailedAuthnChallenge,

    #[error("failed authn response")]
    FailedAuthnResponse,

    #[error("peer failed to send back the proper data value")]
    InvalidPeerData,

    #[error("failed to decrypt challenge from peer")]
    DecryptChallenge(pki::ChallengeError),

    #[error("failed to encrypt data for peer")]
    EncryptData(pki::EncryptError),

    #[error("invalid authn challenge json received from peer")]
    InvalidAuthnChallenge(serde_json::Error),

    #[error(transparent)]
    PrivateKey(#[from] tj2_lib::sec::pki::PrivateKeyError),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Rand(#[from] rand::Error),
}

impl PeerClient {
    pub fn build(peer: UserPeer) -> Result<PeerClientBuilder, reqwest::Error> {
        let origin = if peer.secure {
            format!("https://{}:{}", peer.addr, peer.port)
        } else {
            format!("http://{}:{}", peer.addr, peer.port)
        };
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(peer.ssc)
            .user_agent(CLIENT_USER_AGENT)
            .min_tls_version(reqwest::tls::Version::TLS_1_2)
            .build()?;

        Ok(PeerClientBuilder {
            peer,
            origin,
            client,
        })
    }

    pub fn peer(&self) -> &UserPeer {
        &self.peer
    }

    pub fn get<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>
    {
        self.client.get(origin_url(&self.origin, path))
    }

    pub fn post<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>
    {
        self.client.post(origin_url(&self.origin, path))
    }

    pub fn patch<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>
    {
        self.client.patch(origin_url(&self.origin, path))
    }

    pub fn put<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>
    {
        self.client.put(origin_url(&self.origin, path))
    }
}

impl PeerClientBuilder {
    pub async fn connect(self, storage: &Storage) -> Result<PeerClient, ConnectError> {
        let private_key = self.load_private_key(storage).await?;
        let user_box = ChaChaBox::new(&self.peer.public_key, private_key.secret());

        // generate challenge for peer to authenticate to ensure that they are who
        // we expect
        let data = pki::Data::new()?;

        let api::authn::AuthnChallenge {
            challenge,
            result,
            token,
        } = self.get_challenge(&user_box, &data, private_key.public_key()).await?;

        if result != data {
            return Err(ConnectError::InvalidPeerData);
        }

        tracing::debug!("valid data from peer, processing peer challenge");

        // attempt to decrypt the peer challenge to verify that we are who we say
        // we are
        let authn_response = api::authn::AuthnResponse {
            result: challenge.into_data(&user_box).map_err(|err| ConnectError::DecryptChallenge(err))?,
        };

        let authz_value = ApiSession::authorization_value(&token);

        let res = self.patch("/api/authn")
            .header("authorization", authz_value.clone())
            .json(&authn_response)
            .send()
            .await?;

        // both side have been verified and client can now send data to the peer
        if res.status() != StatusCode::OK {
            Err(ConnectError::FailedAuthnResponse)
        } else {
            let mut default_headers = reqwest::header::HeaderMap::new();
            default_headers.insert("authorization", authz_value);

            let client = reqwest::Client::builder()
                .default_headers(default_headers)
                .danger_accept_invalid_certs(self.peer.ssc)
                .user_agent(CLIENT_USER_AGENT)
                .min_tls_version(reqwest::tls::Version::TLS_1_2)
                .build()?;

            Ok(PeerClient {
                origin: self.origin,
                peer: self.peer,
                client
            })
        }
    }

    async fn load_private_key(&self, storage: &Storage) -> Result<PrivateKey, PrivateKeyError> {
        let dir = storage.user_dir(self.peer.users_id);

        PrivateKey::load(dir.private_key()).await
    }

    async fn get_challenge(
        &self,
        user_box: &ChaChaBox,
        data: &pki::Data,
        public_key: PublicKey,
    ) -> Result<api::authn::AuthnChallenge, ConnectError> {
        let challenge = data.into_challenge(user_box).map_err(|e| ConnectError::EncryptData(e))?;

        let get_authn = api::authn::GetAuthn {
            public_key,
            challenge,
        };

        let res = self.post("/api/authn")
            .json(&get_authn)
            .send()
            .await?;

        if res.status() != StatusCode::OK {
            return Err(ConnectError::FailedAuthnChallenge);
        }

        let bytes = res.bytes().await?;

        // check to make sure that the result is the data we have otherwise do not
        // proceed
        serde_json::from_slice(&bytes).map_err(|err| ConnectError::InvalidAuthnChallenge(err))
    }

    pub fn post<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>
    {
        self.client.post(origin_url(&self.origin, path))
    }

    pub fn patch<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>
    {
        self.client.patch(origin_url(&self.origin, path))
    }
}
