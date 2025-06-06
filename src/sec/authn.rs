use std::convert::Infallible;

use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, Uri};
use axum::response::IntoResponse;

use crate::db;
use crate::net::body;
use crate::net::error::Error as NetError;
use crate::net::header::{is_accepting_html, Location};
use crate::state;
use crate::user;

pub mod session;
pub mod token;

pub use session::{ApiSession, Session};

#[derive(Debug, thiserror::Error)]
pub enum InitiatorError {
    #[error("failed to find the session_id cookie")]
    SessionIdNotFound,

    #[error("failed to find the session from the token")]
    SessionNotFound,

    #[error("failed to find the user for the session")]
    UserNotFound(Session),

    #[error("given session is not authenticated")]
    Unauthenticated(Session),

    #[error("given session is not verified")]
    Unverified(Session),

    #[error("the given session has expired")]
    SessionExpired(Session),

    #[error("failed to parse request header")]
    HeaderStr(#[from] axum::http::header::ToStrError),

    #[error(transparent)]
    Token(#[from] token::InvalidBase64),

    #[error(transparent)]
    DbPg(#[from] db::PgError),
}

#[derive(Debug, thiserror::Error)]
pub enum ApiInitiatorError {
    #[error("missing / invalid authorization header")]
    InvalidAuthorization,

    #[error("failed to find the session from the token")]
    NotFound,

    #[error("failed to find the user for the session")]
    UserNotFound(ApiSession),

    #[error("given session is not authenticated")]
    Unauthenticated(ApiSession),

    #[error("the given session has expired")]
    Expired(ApiSession),

    #[error(transparent)]
    DbPg(#[from] db::PgError),
}

#[derive(Debug)]
pub struct Initiator {
    pub user: user::User,
    pub session: Session,
}

#[derive(Debug)]
pub struct ApiInitiator {
    pub user: user::User,
    pub session: ApiSession,
}

impl Initiator {
    fn get_token(headers: &HeaderMap) -> Result<session::SessionToken, InitiatorError> {
        let token = Session::find_id(headers)?.ok_or(InitiatorError::SessionIdNotFound)?;

        Ok(session::SessionToken::from_base64(token)?)
    }

    fn validate_session(session: session::Session) -> Result<session::Session, InitiatorError> {
        let now = chrono::Utc::now();

        if session.expires_on < now {
            return Err(InitiatorError::SessionExpired(session));
        }

        if !session.authenticated {
            return Err(InitiatorError::Unauthenticated(session));
        }

        if !session.verified {
            return Err(InitiatorError::Unverified(session));
        }

        Ok(session)
    }

    pub async fn from_headers(
        conn: &impl db::GenericClient,
        headers: &HeaderMap,
    ) -> Result<Self, InitiatorError> {
        let token = Self::get_token(headers)?;

        let session = Self::validate_session(
            Session::retrieve_one(conn, &token)
                .await?
                .ok_or(InitiatorError::SessionNotFound)?,
        )?;

        let Some(user) = user::User::retrieve(conn, &session.users_id).await? else {
            return Err(InitiatorError::UserNotFound(session));
        };

        Ok(Self { user, session })
    }
}

impl ApiInitiator {
    fn get_token(headers: &HeaderMap) -> Result<session::ApiSessionToken, ApiInitiatorError> {
        ApiSession::find_token(headers)
            .map_err(|_| ApiInitiatorError::InvalidAuthorization)?
            .ok_or(ApiInitiatorError::InvalidAuthorization)
    }

    fn validate_session(session: ApiSession) -> Result<ApiSession, ApiInitiatorError> {
        let now = chrono::Utc::now();

        if session.expires_on < now {
            return Err(ApiInitiatorError::Expired(session));
        }

        if !session.authenticated {
            return Err(ApiInitiatorError::Unauthenticated(session));
        }

        Ok(session)
    }

    pub async fn from_headers(
        conn: &impl db::GenericClient,
        headers: &HeaderMap,
    ) -> Result<Self, ApiInitiatorError> {
        let token = Self::get_token(headers)?;

        let session = Self::validate_session(
            ApiSession::retrieve_one(conn, &token)
                .await?
                .ok_or(ApiInitiatorError::NotFound)?,
        )?;

        let Some(user) = user::User::retrieve(conn, &session.users_id).await? else {
            return Err(ApiInitiatorError::UserNotFound(session));
        };

        Ok(Self { user, session })
    }
}

pub async fn initiator_redirect<U, E>(
    conn: &impl db::GenericClient,
    headers: &HeaderMap,
    maybe_uri: Option<U>,
) -> Result<Initiator, NetError<E>>
where
    Uri: TryFrom<U>,
{
    match Initiator::from_headers(conn, headers).await {
        Ok(session) => Ok(session),
        Err(err) => {
            if is_accepting_html(headers).unwrap_or(false) {
                match err {
                InitiatorError::Token(_)
                | InitiatorError::SessionIdNotFound
                | InitiatorError::SessionNotFound
                | InitiatorError::UserNotFound(_)
                | InitiatorError::Unauthenticated(_)
                | InitiatorError::Unverified(_)
                | InitiatorError::SessionExpired(_) => Err(NetError::Defined {
                    response: Location::login(maybe_uri).into_response(),
                    msg: None,
                    src: None,
                }),
                InitiatorError::HeaderStr(err) => Err(NetError::Defined {
                    response: body::error_html(Some("There was a problem with information sent in the request. Make sure that your request is properly formatted")).into_response(),
                    msg: None,
                    src: Some(err.into())
                }),
                InitiatorError::DbPg(err) => Err(NetError::Defined {
                    response: body::error_html(None::<&str>).into_response(),
                    msg: None,
                    src: Some(err.into())
                }),
            }
            } else {
                Err(err.into())
            }
        }
    }
}

#[async_trait]
impl FromRequestParts<state::SharedState> for Initiator {
    type Rejection = NetError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &state::SharedState,
    ) -> Result<Self, Self::Rejection> {
        let login_uri = if parts.method == Method::GET {
            Some(parts.uri.clone())
        } else {
            None
        };

        let conn = state.db().get().await?;

        initiator_redirect(&conn, &parts.headers, login_uri).await
    }
}

#[async_trait]
impl FromRequestParts<()> for Initiator {
    type Rejection = Infallible;

    async fn from_request_parts(_: &mut Parts, _: &()) -> Result<Self, Self::Rejection> {
        panic!("no shared state available");
    }
}

#[async_trait]
impl FromRequestParts<state::SharedState> for ApiInitiator {
    type Rejection = NetError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &state::SharedState,
    ) -> Result<Self, Self::Rejection> {
        let conn = state.db().get().await?;

        Ok(ApiInitiator::from_headers(&conn, &parts.headers).await?)
    }
}

#[async_trait]
impl FromRequestParts<()> for ApiInitiator {
    type Rejection = Infallible;

    async fn from_request_parts(_: &mut Parts, _: &()) -> Result<Self, Self::Rejection> {
        panic!("no shared state available");
    }
}
