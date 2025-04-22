use std::net::{Ipv4Addr, Ipv6Addr};

use serde::{Serialize, Deserialize};
use tj2_lib::sec::pki::{PgPublicKey, PublicKey};
use url::Url;

use crate::db;
use crate::db::ids::RemoteServerId;

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
                &[&PgPublicKey(key)]
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
