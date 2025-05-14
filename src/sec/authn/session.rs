use axum::http::{HeaderMap, HeaderValue};
use chrono::Duration;
use chrono::{DateTime, Utc};

use crate::db;
use crate::db::ids::{UserId, UserClientId};
use crate::cookie;
use crate::sec::authn::token::{Token, InvalidBase64};

pub const API_SESSION_ID_KEY: &str = "api_session_id";
pub const API_SESSION_TOKEN_LEN: usize = 48;
pub const SESSION_ID_KEY: &str = "session_id";
pub const SESSION_TOKEN_LEN: usize = 48;

pub type ApiSessionToken = Token<API_SESSION_TOKEN_LEN>;
pub type SessionToken = Token<SESSION_TOKEN_LEN>;

#[derive(Debug, Clone)]
pub struct Session {
    pub token: SessionToken,
    pub users_id: UserId,
    pub issued_on: DateTime<Utc>,
    pub expires_on: DateTime<Utc>,
    pub authenticated: bool,
    pub verified: bool,
}

#[derive(Debug, Clone)]
pub struct ApiSession {
    pub token: ApiSessionToken,
    pub users_id: UserId,
    pub user_clients_id: UserClientId,
    pub issued_on: DateTime<Utc>,
    pub expires_on: DateTime<Utc>,
    pub authenticated: bool,
}

#[derive(Debug)]
pub struct SessionOptions {
    pub users_id: UserId,
    pub duration: Duration,
    pub authenticated: bool,
    pub verified: bool,
}

#[derive(Debug)]
pub struct ApiSessionOptions {
    pub users_id: UserId,
    pub user_clients_id: UserClientId,
    pub duration: Duration,
    pub authenticated: bool,
}

#[derive(Debug)]
pub enum SessionRetrieveOne<'a> {
    Token(&'a SessionToken)
}

#[derive(Debug)]
pub enum ApiSessionRetrieveOne<'a> {
    Token(&'a ApiSessionToken)
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("the specified token already exists")]
    TokenExists,

    #[error("the specified user was not found")]
    UserNotFound,

    #[error("the expires_on timestamp overflowed")]
    ExpiresOnOverflow,

    #[error(transparent)]
    Db(#[from] db::PgError),

    #[error(transparent)]
    Rand(#[from] rand::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ApiSessionError {
    #[error("the specified token already exists")]
    TokenExists,

    #[error("the specified user was not found")]
    UserNotFound,

    #[error("the expires_on timestamp overflowed")]
    ExpiresOnOverflow,

    #[error("the specified user client was not found")]
    UserClientNotFound,

    #[error(transparent)]
    Header(#[from] axum::http::header::ToStrError),

    #[error(transparent)]
    Token(#[from] InvalidBase64),

    #[error(transparent)]
    Db(#[from] db::PgError),

    #[error(transparent)]
    Rand(#[from] rand::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum AuthHeaderError {
    #[error("invalid authorization header scheme")]
    InvalidScheme,

    #[error("invalid authorization header format")]
    InvalidFormat,

    #[error("invalid authorization header value")]
    InvalidValue,

    #[error(transparent)]
    Header(#[from] axum::http::header::ToStrError),
}

impl Session {
    pub fn find_id(headers: &HeaderMap) -> Result<Option<&str>, axum::http::header::ToStrError> {
        crate::net::cookie::find_cookie_value(headers, SESSION_ID_KEY)
    }

    pub async fn create(
        conn: &impl db::GenericClient,
        SessionOptions {
            users_id,
            duration,
            authenticated,
            verified,
        }: SessionOptions
    ) -> Result<Self, SessionError> {
        let token = SessionToken::new()?;
        let issued_on = Utc::now();
        let expires_on = issued_on.checked_add_signed(duration)
            .ok_or(SessionError::ExpiresOnOverflow)?;

        let result = conn.execute(
            "\
            insert into authn_sessions (token, users_id, issued_on, expires_on, authenticated, verified) values \
            ($1, $2, $3, $4, $5, $6)",
            &[&token, &users_id, &issued_on, &expires_on, &authenticated, &verified]
        ).await;

        match result {
            Ok(_) => Ok(Self {
                token,
                users_id,
                issued_on,
                expires_on,
                authenticated,
                verified,
            }),
            Err(err) => if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::Unique(constraint) => match constraint {
                        "authn_sessions_pkey" => Err(SessionError::TokenExists),
                        _ => Err(SessionError::Db(err))
                    }
                    db::ErrorKind::ForeignKey(constraint) => match constraint {
                        "authn_sessions_users_id_fkey" => Err(SessionError::UserNotFound),
                        _ => Err(SessionError::Db(err))
                    }
                }
            } else {
                Err(SessionError::Db(err))
            }
        }
    }

    pub async fn retrieve_one<'a, T>(conn: &impl db::GenericClient, given: T) -> Result<Option<Self>, db::PgError>
    where
        T: Into<SessionRetrieveOne<'a>>
    {
        Ok(match given.into() {
            SessionRetrieveOne::Token(token) => conn.query_opt(
                "\
                select token, \
                       users_id, \
                       issued_on, \
                       expires_on, \
                       authenticated, \
                       verified \
                from authn_sessions \
                where token = $1",
                &[token]
            ).await?
        }.map(|row| Self {
            token: row.get(0),
            users_id: row.get(1),
            issued_on: row.get(2),
            expires_on: row.get(3),
            authenticated: row.get(4),
            verified: row.get(5),
        }))
    }

    pub async fn update(&self, conn: &impl db::GenericClient) -> Result<bool, db::PgError> {
        let result = conn.execute(
            "\
            update authn_sessions \
            set expires_on = $2, \
                verified = $3 \
            where token = $1",
            &[&self.token, &self.expires_on, &self.verified]
        ).await?;

        Ok(result == 1)
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

impl ApiSession {
    pub fn authorization_value(token: &ApiSessionToken) -> HeaderValue {
        let value = format!("tj2-token {}", token.as_base64());

        let mut rtn = HeaderValue::from_str(&value).unwrap();
        rtn.set_sensitive(true);

        rtn
    }

    pub fn find_token(headers: &HeaderMap) -> Result<Option<ApiSessionToken>, AuthHeaderError> {
        let Some(auth_header) = headers.get("Authorization") else {
            return Ok(None);
        };

        let auth_str = auth_header.to_str()?;

        let (scheme, value) = auth_str.split_once(' ')
            .ok_or(AuthHeaderError::InvalidFormat)?;

        if scheme != "tj2-token" {
            return Err(AuthHeaderError::InvalidScheme);
        }

        let token = ApiSessionToken::from_base64(value)
            .map_err(|_| AuthHeaderError::InvalidValue)?;

        Ok(Some(token))
    }

    pub async fn create(
        conn: &impl db::GenericClient,
        ApiSessionOptions {
            users_id,
            user_clients_id,
            duration,
            authenticated,
        }: ApiSessionOptions
    ) -> Result<Self, ApiSessionError> {
        let token = SessionToken::new()?;
        let issued_on = Utc::now();
        let expires_on = issued_on.checked_add_signed(duration)
            .ok_or(ApiSessionError::ExpiresOnOverflow)?;

        let result = conn.execute(
            "\
            insert into authn_api_sessions (token, users_id, user_clients_id, issued_on, expires_on, authenticated) values \
            ($1, $2, $3, $4, $5, $6)",
            &[&token, &users_id, &user_clients_id, &issued_on, &expires_on, &authenticated]
        ).await;

        match result {
            Ok(_) => Ok(Self {
                token,
                users_id,
                user_clients_id,
                issued_on,
                expires_on,
                authenticated,
            }),
            Err(err) => if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::Unique(constraint) => match constraint {
                        "authn_api_sessions_pkey" => Err(ApiSessionError::TokenExists),
                        _ => Err(ApiSessionError::Db(err))
                    }
                    db::ErrorKind::ForeignKey(constraint) => match constraint {
                        "authn_api_sessions_users_id_fkey" => Err(ApiSessionError::UserNotFound),
                        "authn_api_sessions_user_clients_id_fkey" => Err(ApiSessionError::UserClientNotFound),
                        _ => Err(ApiSessionError::Db(err))
                    }
                }
            } else {
                Err(ApiSessionError::Db(err))
            }
        }
    }

    pub async fn retrieve_one<'a, T>(conn: &impl db::GenericClient, given: T) -> Result<Option<Self>, db::PgError>
    where
        T: Into<ApiSessionRetrieveOne<'a>>
    {
        Ok(match given.into() {
            ApiSessionRetrieveOne::Token(token) => conn.query_opt(
                "\
                select token, \
                       users_id, \
                       user_clients_id, \
                       issued_on, \
                       expires_on, \
                       authenticated \
                from authn_api_sessions \
                where token = $1",
                &[token]
            ).await?
        }.map(|row| Self {
            token: row.get(0),
            users_id: row.get(1),
            user_clients_id: row.get(2),
            issued_on: row.get(3),
            expires_on: row.get(4),
            authenticated: row.get(5),
        }))
    }

    pub async fn update(&self, conn: &impl db::GenericClient) -> Result<bool, db::PgError> {
        let result = conn.execute(
            "\
            update authn_api_sessions \
            set expires_on = $2, \
                authenticated = $3 \
            where token = $1",
            &[&self.token, &self.expires_on, &self.authenticated]
        ).await?;

        Ok(result == 1)
    }

    pub async fn delete(&self, conn: &impl db::GenericClient) -> Result<bool, db::PgError> {
        let result = conn.execute(
            "delete from authn_api_sessions where token = $1",
            &[&self.token]
        ).await?;

        Ok(result == 1)
    }
}

impl SessionOptions {
    pub fn new<I>(users_id: I) -> Self
    where
        I: Into<UserId>
    {
        Self {
            users_id: users_id.into(),
            duration: Duration::days(7),
            authenticated: false,
            verified: false,
        }
    }
}

impl ApiSessionOptions {
    pub fn new<I, C>(users_id: I, user_clients_id: C) -> Self
    where
        I: Into<UserId>,
        C: Into<UserClientId>,
    {
        Self {
            users_id: users_id.into(),
            user_clients_id: user_clients_id.into(),
            duration: Duration::days(7),
            authenticated: false,
        }
    }
}

impl<'a> From<&'a SessionToken> for SessionRetrieveOne<'a> {
    fn from(token: &'a SessionToken) -> Self {
        Self::Token(token)
    }
}

impl<'a> From<&'a ApiSessionToken> for ApiSessionRetrieveOne<'a> {
    fn from(token: &'a ApiSessionToken) -> Self {
        Self::Token(token)
    }
}
