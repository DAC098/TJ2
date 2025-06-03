use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::net::Error as NetError;
use crate::router::{body, macros};
use crate::sec::authn::{Initiator, InitiatorError};
use crate::sec::otp;
use crate::state;

pub async fn get(
    state: state::SharedState,
    headers: HeaderMap,
) -> Result<impl IntoResponse, NetError> {
    macros::res_if_html!(state.templates(), &headers);

    Ok(body::Json("okay").into_response())
}

#[derive(Debug, Deserialize)]
pub struct VerifyBody {
    code: String,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum VerifyError {
    MFANotFound,
    InvalidCode,
    AlreadyVerified,
    InvalidSession,
}

impl IntoResponse for VerifyError {
    fn into_response(self) -> Response {
        match self {
            Self::InvalidSession | Self::InvalidCode | Self::AlreadyVerified => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
            Self::MFANotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn post(
    state: state::SharedState,
    headers: HeaderMap,
    body::Json(verify): body::Json<VerifyBody>,
) -> Result<impl IntoResponse, NetError<VerifyError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let mut session = match Initiator::from_headers(&transaction, &headers).await {
        Ok(_) => return Err(NetError::Inner(VerifyError::AlreadyVerified)),
        Err(err) => match err {
            InitiatorError::Unverified(session) => session,
            InitiatorError::DbPg(err) => return Err(err.into()),
            _ => return Err(NetError::Inner(VerifyError::InvalidSession)),
        },
    };

    let totp = otp::Totp::retrieve(&transaction, &session.users_id)
        .await?
        .ok_or(NetError::Inner(VerifyError::MFANotFound))?;

    if !totp.verify(&verify.code)? {
        return Err(NetError::Inner(VerifyError::InvalidCode));
    }

    session.verified = true;

    session.update(&transaction).await?;

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
