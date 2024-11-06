use std::fmt;

use axum::http::HeaderMap;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bytes::BytesMut;
use chrono::Duration;
use chrono::{DateTime, Utc};
use rand::RngCore;
use postgres_types as pg_types;

use crate::error::{self, Context, BoxDynError};
use crate::db;
use crate::cookie;

pub const SESSION_ID_KEY: &str = "session_id";
pub const SESSION_TOKEN_LEN: usize = 48;

#[derive(Debug, thiserror::Error)]
#[error("invalid base64 string provided")]
pub struct InvalidBase64;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Token([u8; SESSION_TOKEN_LEN]);

impl Token {
    pub fn empty() -> Self {
        Token([0; SESSION_TOKEN_LEN])
    }

    pub fn new() -> Result<Self, rand::Error> {
        let mut bytes = [0; SESSION_TOKEN_LEN];

        rand::thread_rng().try_fill_bytes(&mut bytes)?;

        Ok(Token(bytes))
    }

    pub fn from_base64(given: &str) -> Result<Self, InvalidBase64> {
        let decoded = URL_SAFE_NO_PAD.decode(given)
            .map_err(|_| InvalidBase64)?;

        let bytes = decoded.try_into()
            .map_err(|_| InvalidBase64)?;

        Ok(Token(bytes))
    }

    pub fn as_base64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }

        Ok(())
    }
}

impl pg_types::ToSql for Token {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        self.0.as_slice()
            .to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&[u8] as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl<'a> pg_types::FromSql<'a> for Token {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let v = <Vec<u8> as pg_types::FromSql>::from_sql(ty, raw)?;

        let Ok(bytes) = v.try_into() else {
            return Err("invalid sql value for Token. expected bytea with 48 bytes".into());
        };

        Ok(Token(bytes))
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&[u8] as pg_types::FromSql>::accepts(ty)
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    pub token: Token,
    pub users_id: db::ids::UserId,
    pub dropped: bool,
    pub issued_on: DateTime<Utc>,
    pub expires_on: DateTime<Utc>,
    pub authenticated: bool,
    pub verified: bool,
}

pub struct SessionOptions {
    pub users_id: db::ids::UserId,
    pub duration: Duration,
    pub authenticated: bool,
    pub verified: bool,
}

impl SessionOptions {
    pub fn new<I>(users_id: I) -> Self
    where
        I: Into<db::ids::UserId>
    {
        SessionOptions {
            users_id: users_id.into(),
            duration: Duration::days(7),
            authenticated: false,
            verified: false,
        }
    }
}

impl Session {
    pub async fn create(conn: &impl db::GenericClient, options: SessionOptions) -> Result<Self, error::Error> {
        let users_id = options.users_id;
        let dropped = false;
        let issued_on = Utc::now();
        let expires_on = issued_on.checked_add_signed(options.duration)
            .context("failed to add duration to expires_on")?;
        let authenticated = options.authenticated;
        let verified = options.verified;
        let mut attempts = 3usize;
        let mut token: Token;

        loop {
            attempts -= 1;
            token = Token::new()
                .context("failed to create token")?;

            let result = conn.execute(
                "\
                insert into authn_sessions (token, users_id, issued_on, expires_on, authenticated, verified) values \
                ($1, $2, $3, $4, $5, $6)",
                &[&token, &users_id, &issued_on, &expires_on, &authenticated, &verified]
            )
                .await
                .context("failed to insert session")?;

            if result == 0 {
                if attempts == 0 {
                    return Err(error::Error::context("failed to insert session"));
                }
            } else {
                break;
            }
        }

        Ok(Self {
            token,
            users_id,
            dropped,
            issued_on,
            expires_on,
            authenticated,
            verified,
        })
    }

    pub async fn retrieve_token(conn: &impl db::GenericClient, token: &Token) -> Result<Option<Self>, db::PgError> {
        let maybe = conn.query_opt(
            "\
            select token, \
                   users_id, \
                   dropped, \
                   issued_on, \
                   expires_on, \
                   authenticated, \
                   verified \
            from authn_sessions \
            where token = $1",
            &[token]
        ).await?;

        if let Some(row) = maybe {
            Ok(Some(Self {
                token: row.get(0),
                users_id: row.get(1),
                dropped: row.get(2),
                issued_on: row.get(3),
                expires_on: row.get(4),
                authenticated: row.get(5),
                verified: row.get(6),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn delete(&self, conn: &impl db::GenericClient) -> Result<bool, db::PgError> {
        let result = conn.execute(
            "delete from authn_sessions where token = $1",
            &[&self.token]
        ).await?;

        Ok(result == 1)
    }

    pub fn build_cookie(&self) -> cookie::SetCookie {
        cookie::SetCookie::new(SESSION_ID_KEY, self.token.as_base64())
            .with_expires(self.expires_on)
            .with_path("/")
            .with_secure(true)
            .with_http_only(true)
            .with_same_site(cookie::SameSite::Strict)
    }

    pub fn clear_cookie() -> cookie::SetCookie {
        cookie::SetCookie::new(SESSION_ID_KEY, "")
            .with_max_age(std::time::Duration::from_secs(0))
            .with_path("/")
            .with_secure(true)
            .with_http_only(true)
            .with_same_site(cookie::SameSite::Strict)
    }
}

pub fn find_session_id(headers: &HeaderMap) -> Result<Option<&str>, axum::http::header::ToStrError> {
    for cookie in headers.get_all("cookie") {
        let cookie_str = cookie.to_str()?;

        for sub_cookie in cookie_str.split("; ") {
            let Some((key, value)) = sub_cookie.split_once('=') else {
                continue;
            };

            if key == SESSION_ID_KEY {
                return Ok(Some(value))
            }
        }
    }

    Ok(None)
}
