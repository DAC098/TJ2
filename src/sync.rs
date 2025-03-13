use crate::db;
use crate::db::ids::RemoteServerId;

pub mod journal;

pub struct RemoteServer {
    id: RemoteServerId,
    addr: String,
    port: u16,
    secure: bool,
}

impl RemoteServer {
    fn get_port(given: i32) -> u16 {
        match given.try_into() {
            Ok(valid) => valid,
            Err(_) => panic!("invalid port value from remote server database record. value: {given}"),
        }
    }

    pub async fn retrieve(
        conn: &impl db::GenericClient,
        id: &RemoteServerId
    ) -> Result<Option<Self>, db::PgError> {
        conn.query_opt(
            "\
            select remote_servers.addr, \
                   remote_servers.port, \
                   remote_servers.secure \
            from remote_servers \
            where remote_servers.id = $1",
            &[id]
        )
            .await
            .map(|result| result.map(|row| Self {
                id: *id,
                addr: row.get(0),
                port: Self::get_port(row.get(1)),
                secure: row.get(2),
            }))
    }

    pub fn id(&self) -> &RemoteServerId {
        &self.id
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
