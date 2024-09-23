use std::borrow::Cow;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Duration;
use chrono::{DateTime, Utc};
use rand::RngCore;
use sqlx::{Row, Type, Encode, Decode, Sqlite};
use sqlx::encode::IsNull;
use sqlx::sqlite::{SqliteTypeInfo, SqliteValueRef, SqliteArgumentValue};

use crate::error::{self, Context, BoxDynError};
use crate::db;
use crate::cookie;

pub const SESSION_TOKEN_LEN: usize = 48;

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("invalid base64 string provided")]
    InvalidBase64,

    #[error(transparent)]
    Rand(#[from] rand::Error),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Token([u8; SESSION_TOKEN_LEN]);

impl Token {
    pub fn empty() -> Self {
        Token([0; SESSION_TOKEN_LEN])
    }

    pub fn new() -> Result<Self, TokenError> {
        let mut bytes = [0; SESSION_TOKEN_LEN];

        rand::thread_rng().try_fill_bytes(&mut bytes)?;

        Ok(Token(bytes))
    }

    pub fn from_base64(given: &str) -> Result<Self, TokenError> {
        let decoded = URL_SAFE_NO_PAD.decode(given)
            .map_err(|_| TokenError::InvalidBase64)?;

        let bytes = decoded.try_into()
            .map_err(|_| TokenError::InvalidBase64)?;

        Ok(Token(bytes))
    }

    pub fn as_base64(&self) -> String {
        URL_SAFE_NO_PAD.encode(&self.0)
    }
}

impl<'a> Encode<'a, Sqlite> for &'a Token {
    fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'a>>) -> Result<IsNull, BoxDynError> {
        let buf_cow = Cow::Borrowed(self.0.as_slice());

        buf.push(SqliteArgumentValue::Blob(buf_cow));

        Ok(IsNull::No)
    }
}

impl<'a> Decode<'a, Sqlite> for Token {
    fn decode(value: SqliteValueRef<'a>) -> Result<Self, BoxDynError> {
        let slice = <&[u8] as Decode<Sqlite>>::decode(value)?;

        let Ok(result) = TryFrom::try_from(slice) else {
            return Err("invalid blob received from database for token".into());
        };

        Ok(Token(result))
    }
}

impl Type<Sqlite> for Token {
    fn type_info() -> SqliteTypeInfo {
        <&[u8] as Type<Sqlite>>::type_info()
    }
}

pub struct Session {
    pub token: Token,
    pub users_id: i64,
    pub dropped: bool,
    pub issued_on: DateTime<Utc>,
    pub expires_on: DateTime<Utc>,
    pub authenticated: bool,
    pub verified: bool,
}

impl Session {
    pub async fn create(conn: &mut db::DbConn, users_id: i64, duration: Duration) -> Result<Self, error::Error> {
        let mut token: Token;
        let dropped = false;
        let issued_on = Utc::now();
        let expires_on = issued_on.clone()
            .checked_add_signed(duration)
            .unwrap();
        let authenticated = false;
        let verified = false;
        let mut attempts = 3usize;

        loop {
            attempts -= 1;
            token = Token::new()
                .context("failed to create token")?;

            let result = sqlx::query(
                "\
                insert into authn_sessions (token, users_id, issued_on, expires_on) \
                values (?1, ?2, ?3, ?4)"
            )
                .bind(&token)
                .bind(&users_id)
                .bind(&issued_on)
                .bind(&expires_on)
                .execute(&mut *conn)
                .await;

            if let Err(err) = result {
                match err {
                    sqlx::Error::Database(ref db) => {
                        if attempts == 0 {
                            return Err(error::Error::context_source("failed to insert session", err));
                        }

                        tracing::debug!("database error: kind: {:?}\n{err:#?}", db.kind());

                        if !matches!(db.kind(), sqlx::error::ErrorKind::UniqueViolation) {
                            return Err(error::Error::context_source("failed to insert session", err));
                        }

                        // for sqlite the constraint method will always return
                        // so we have to do checks on the string
                        if !db.message().ends_with("authn_sessions.token") {
                            return Err(error::Error::context_source("failed to insert session", err));
                        }
                    },
                    err => return Err(error::Error::context_source("failed to insert session", err))
                }
            } else {
                break;
            }
        }

        Ok(Session {
            token,
            users_id,
            dropped,
            issued_on,
            expires_on,
            authenticated,
            verified,
        })
    }

    pub async fn retrieve_token(conn: &mut db::DbConn, token: &Token) -> Result<Option<Self>, error::Error> {
        let maybe = sqlx::query("select * from authn_sessions where token = ?1")
            .bind(token)
            .fetch_optional(&mut *conn)
            .await
            .context("failed to retrieve authn_session record from database")?;

        if let Some(row) = maybe {
            Ok(Some(Session {
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

    pub fn build_cookie(&self) -> cookie::SetCookie {
        cookie::SetCookie::new("session_id", self.token.as_base64())
            .with_expires(self.expires_on.clone())
            .with_path("/")
            .with_secure(true)
            .with_http_only(true)
            .with_same_site(cookie::SameSite::Strict)
    }
}

