use axum::http::StatusCode;
use crypto_box::ChaChaBox;
use tj2_lib::sec::pki::{PrivateKey, PrivateKeyError, PublicKey};

use crate::api;
use crate::sec::authn::ApiSession;
use crate::sec::pki;
use crate::state::Storage;
use crate::user::peer::UserPeer;

pub mod journal;

static CLIENT_USER_AGENT: &str = "tj2-sync-client/0.0.1";

fn origin_url<P>(origin: &str, path: P) -> String
where
    P: AsRef<str>,
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

    pub fn post<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>,
    {
        self.client.post(origin_url(&self.origin, path))
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
        } = self
            .get_challenge(&user_box, &data, private_key.public_key())
            .await?;

        if result != data {
            return Err(ConnectError::InvalidPeerData);
        }

        tracing::debug!("valid data from peer, processing peer challenge");

        // attempt to decrypt the peer challenge to verify that we are who we say
        // we are
        let authn_response = api::authn::AuthnResponse {
            result: challenge
                .into_data(&user_box)
                .map_err(|err| ConnectError::DecryptChallenge(err))?,
        };

        let authz_value = ApiSession::authorization_value(&token);

        let res = self
            .patch("/api/authn")
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
                client,
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
        let challenge = data
            .into_challenge(user_box)
            .map_err(|e| ConnectError::EncryptData(e))?;

        let get_authn = api::authn::GetAuthn {
            public_key,
            challenge,
        };

        let res = self.post("/api/authn").json(&get_authn).send().await?;

        if res.status() != StatusCode::OK {
            return Err(ConnectError::FailedAuthnChallenge);
        }

        let bytes = res.bytes().await?;

        // check to make sure that the result is the data we have otherwise do not
        // proceed
        serde_json::from_slice(&bytes).map_err(|err| ConnectError::InvalidAuthnChallenge(err))
    }

    fn post<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>,
    {
        self.client.post(origin_url(&self.origin, path))
    }

    fn patch<P>(&self, path: P) -> reqwest::RequestBuilder
    where
        P: AsRef<str>,
    {
        self.client.patch(origin_url(&self.origin, path))
    }
}
