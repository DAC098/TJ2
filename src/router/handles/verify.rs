use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::db::ids::UserId;
use crate::net::body;
use crate::net::Error as NetError;
use crate::sec::authn::{Initiator, InitiatorError};
use crate::sec::mfa::{self, otp};
use crate::state;
use crate::user::User;

pub async fn get(
    state: state::SharedState,
    headers: HeaderMap,
) -> Result<body::Json<&'static str>, NetError> {
    body::assert_html(state.templates(), &headers)?;

    Ok(body::Json("okay"))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum VerifyBody {
    Totp { code: String },
    Recovery { code: String },
}

#[derive(Debug, Serialize)]
struct VerifySuccess {
    id: UserId,
    username: String,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum VerifyError {
    MFANotFound,
    InvalidCode,
    InvalidRecovery,
    AlreadyVerified,
    InvalidSession,
    UserNotFound,
}

impl IntoResponse for VerifyError {
    fn into_response(self) -> Response {
        match self {
            Self::InvalidSession
            | Self::InvalidCode
            | Self::AlreadyVerified
            | Self::InvalidRecovery => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::MFANotFound | Self::UserNotFound => {
                (StatusCode::NOT_FOUND, body::Json(self)).into_response()
            }
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

    match verify {
        VerifyBody::Totp { code } => {
            let totp = otp::Totp::retrieve(&transaction, &session.users_id)
                .await?
                .ok_or(NetError::Inner(VerifyError::MFANotFound))?;

            if !totp.verify(&code)? {
                return Err(NetError::Inner(VerifyError::InvalidCode));
            }
        }
        VerifyBody::Recovery { code } => {
            if !otp::Totp::exists(&transaction, &session.users_id).await? {
                return Err(NetError::Inner(VerifyError::MFANotFound));
            }

            if !mfa::verify_and_mark(&transaction, &session.users_id, &code).await? {
                return Err(NetError::Inner(VerifyError::InvalidRecovery));
            }
        }
    }

    let User { id, username, .. } = User::retrieve(&transaction, &session.users_id)
        .await?
        .ok_or(NetError::Inner(VerifyError::UserNotFound))?;

    session.verified = true;

    session.update(&transaction).await?;

    transaction.commit().await?;

    Ok(body::Json(VerifySuccess { id, username }))
}
