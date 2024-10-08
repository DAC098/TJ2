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
    Db(#[from] sqlx::Error),
}

#[derive(Debug)]
pub struct Initiator {
    pub user: user::User,
    pub session: Session,
}

impl Initiator {
    pub async fn from_headers(
        conn: &mut db::DbConn,
        headers: &HeaderMap,
    ) -> Result<Self, InitiatorError> {
        let Some(session_id) = session::find_session_id(headers)? else {
            return Err(InitiatorError::SessionIdNotFound);
        };

        let token = session::Token::from_base64(session_id)?;

        let Some(session) = Session::retrieve_token(conn, &token).await? else {
            return Err(InitiatorError::SessionNotFound);
        };

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

        let Some(user) = user::User::retrieve_id(&mut *conn, session.users_id).await? else {
            return Err(InitiatorError::UserNotFound(session));
        };

        Ok(Initiator {
            user,
            session,
        })
    }
}
