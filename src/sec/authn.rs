use axum::http::HeaderMap;

use crate::db;
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

    #[error(transparent)]
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

        tracing::debug!("retrieving session for {token}");

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
