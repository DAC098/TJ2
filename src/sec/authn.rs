use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::{Uri, HeaderMap, Method, StatusCode};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::db;
use crate::error::{self, Context};
use crate::header::{Location, is_accepting_html};
use crate::router::body;
use crate::state;
use crate::user;

pub mod token;
pub mod session;

pub use session::{Session, ApiSession};

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
        headers: &HeaderMap
    ) -> Result<Self, InitiatorError> {
        let token = Self::get_token(headers)?;

        let session = Self::validate_session(
            Session::retrieve_one(conn, &token)
                .await?
                .ok_or(InitiatorError::SessionNotFound)?
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

    pub async fn from_headers(conn: &impl db::GenericClient, headers: &HeaderMap) -> Result<Self, ApiInitiatorError> {
        let token = Self::get_token(headers)?;

        let session = Self::validate_session(
            ApiSession::retrieve_one(conn, &token)
                .await?
                .ok_or(ApiInitiatorError::NotFound)?
        )?;

        let Some(user) = user::User::retrieve(conn, &session.users_id).await? else {
            return Err(ApiInitiatorError::UserNotFound(session));
        };

        Ok(Self { user, session })
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum FromReqPartsJsonError {
    InvalidSession,
    InvalidRequest,
}

impl IntoResponse for FromReqPartsJsonError {
    fn into_response(self) -> Response {
        match &self {
            Self::InvalidSession => (
                StatusCode::UNAUTHORIZED,
                body::Json(self)
            ).into_response(),
            Self::InvalidRequest => (
                StatusCode::BAD_REQUEST,
                body::Json(self)
            ).into_response()
        }
    }
}

pub enum FromReqPartsError {
    Login(Option<Uri>),
    Json(FromReqPartsJsonError),
    Error(error::Error),
}

impl IntoResponse for FromReqPartsError {
    fn into_response(self) -> Response {
        match self {
            Self::Login(redir) => Location::login(redir).into_response(),
            Self::Json(json) => json.into_response(),
            Self::Error(err) => err.into_response(),
        }
    }
}

impl From<error::Error> for FromReqPartsError {
    fn from(err: error::Error) -> Self {
        Self::Error(err)
    }
}

#[async_trait]
impl FromRequestParts<state::SharedState> for Initiator {
    type Rejection = FromReqPartsError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &state::SharedState
    ) -> Result<Self, Self::Rejection> {
        let is_html = is_accepting_html(&parts.headers).unwrap_or(false);
        let login_uri = if parts.method == Method::GET {
            Some(parts.uri.clone())
        } else {
            None
        };

        let conn = state.db()
            .get()
            .await
            .context("failed to retrieve database connection")?;

        match Self::from_headers(&conn, &parts.headers).await {
            Ok(session) => Ok(session),
            Err(err) => match err {
                InitiatorError::Token(_) |
                InitiatorError::SessionIdNotFound |
                InitiatorError::SessionNotFound |
                InitiatorError::UserNotFound(_) |
                InitiatorError::Unauthenticated(_) |
                InitiatorError::Unverified(_) |
                InitiatorError::SessionExpired(_) => if is_html {
                    Err(FromReqPartsError::Login(login_uri))
                } else {
                    Err(FromReqPartsError::Json(
                        FromReqPartsJsonError::InvalidSession
                    ))
                },
                InitiatorError::HeaderStr(_) => if is_html {
                    Err(FromReqPartsError::Login(login_uri))
                } else {
                    Err(FromReqPartsError::Json(
                        FromReqPartsJsonError::InvalidRequest
                    ))
                },
                InitiatorError::DbPg(err) => Err(error::Error::context_source(
                    "database error when retrieving session",
                    err
                ).into())
            }
        }
    }
}

#[async_trait]
impl FromRequestParts<state::SharedState> for ApiInitiator {
    type Rejection = error::Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &state::SharedState
    ) -> Result<Self, Self::Rejection> {
        let conn = state.db()
            .get()
            .await
            .context("failed to get db connection")?;

        ApiInitiator::from_headers(&conn, &parts.headers)
            .await
            .context("failed to retrieve initiator")
    }
}
