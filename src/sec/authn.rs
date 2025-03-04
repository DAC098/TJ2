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

pub mod session;
pub use session::Session;

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
    Token(#[from] session::InvalidBase64),

    #[error(transparent)]
    DbPg(#[from] db::PgError),
}

#[derive(Debug)]
pub struct Initiator {
    pub user: user::User,
    pub session: Session,
}

impl Initiator {
    fn get_token(headers: &HeaderMap) -> Result<session::Token, InitiatorError> {
        let Some(session_id) = session::find_session_id(headers)? else {
            return Err(InitiatorError::SessionIdNotFound);
        };

        Ok(session::Token::from_base64(session_id)?)
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

        let Some(session) = Session::retrieve_token(conn, &token).await? else {
            return Err(InitiatorError::SessionNotFound);
        };

        let session = Self::validate_session(session)?;

        let Some(user) = user::User::retrieve_id(conn, session.users_id).await? else {
            return Err(InitiatorError::UserNotFound(session));
        };

        Ok(Initiator {
            user,
            session
        })
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
