use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error;
use crate::router::{body, macros};
use crate::sec::authn::{Initiator, InitiatorError};
use crate::sec::otp;
use crate::state;

pub async fn get(
    state: state::SharedState,
    headers: HeaderMap
) -> Result<impl IntoResponse, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    Ok(body::Json("okay").into_response())
}

#[derive(Debug, Deserialize)]
pub struct VerifyBody {
    code: String
}

#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "type")]
pub enum VerifyError {
    #[error("no MFA method was found")]
    MFANotFound,

    #[error("invalid totp code")]
    InvalidCode,

    #[error("user has already been verified")]
    AlreadyVerified,

    #[error("invalid session id")]
    InvalidSession,

    #[serde(skip)]
    #[error(transparent)]
    Db(#[from] db::PgError),

    #[serde(skip)]
    #[error(transparent)]
    DbPool(#[from] db::PoolError),

    #[serde(skip)]
    #[error(transparent)]
    Error(#[from] error::Error),

    #[serde(skip)]
    #[error(transparent)]
    UnixTimestamp(#[from] otp::UnixTimestampError),
}

impl IntoResponse for VerifyError {
    fn into_response(self) -> Response {
        error::log_prefix_error("response error", &self);

        match self {
            Self::InvalidSession |
            Self::InvalidCode |
            Self::AlreadyVerified => (
                StatusCode::BAD_REQUEST,
                body::Json(self)
            ).into_response(),
            Self::MFANotFound => (
                StatusCode::NOT_FOUND,
                body::Json(self),
            ).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

pub async fn post(
    state: state::SharedState,
    headers: HeaderMap,
    body::Json(verify): body::Json<VerifyBody>
) -> Result<impl IntoResponse, VerifyError> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let mut session = match Initiator::from_headers(&transaction, &headers).await {
        Ok(_) => return Err(VerifyError::AlreadyVerified),
        Err(err) => match err {
            InitiatorError::Unverified(session) => session,
            InitiatorError::DbPg(err) => return Err(VerifyError::Db(err)),
            _ => return Err(VerifyError::InvalidSession),
        }
    };

    let totp = otp::Totp::retrieve(&transaction, &session.users_id)
        .await?
        .ok_or(VerifyError::MFANotFound)?;

    if !totp.verify(&verify.code)? {
        return Err(VerifyError::InvalidCode);
    }

    session.verified = true;

    session.update(&transaction).await?;

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
