use axum::http::{StatusCode, HeaderMap};
use axum::response::{Response, IntoResponse};
use crypto_box::ChaChaBox;
use serde::{Serialize, Deserialize};
use tj2_lib::sec::pki::{PrivateKey, PrivateKeyError};

use crate::api;
use crate::db;
use crate::error;
use crate::state;
use crate::router::body;
use crate::user::client::UserClient;
use crate::sec::authn::{ApiInitiator, ApiInitiatorError, ApiSession};
use crate::sec::authn::session::{ApiSessionOptions, ApiSessionError};
use crate::sec::pki::{Data, EncryptError};

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum AuthnError {
    #[error("the requested public was not found")]
    KeyNotFound,

    #[error("invalid client challenge provided")]
    ClientChallenge,

    #[error("missing / invalid authorization header")]
    InvalidAuthorization,

    #[error("api session not found")]
    SessionNotFound,

    #[error("user not found")]
    UserNotFound,

    #[error("data for token was not found")]
    DataNotFound,

    #[error("expired session")]
    Expired,

    #[error("invalid data")]
    InvalidData,

    #[serde(skip)]
    #[error(transparent)]
    Encrypt(#[from] EncryptError),

    #[serde(skip)]
    #[error(transparent)]
    Db(#[from] db::PgError),

    #[serde(skip)]
    #[error(transparent)]
    DbPool(#[from] db::PoolError),

    #[serde(skip)]
    #[error(transparent)]
    PrivateKey(#[from] PrivateKeyError),

    #[serde(skip)]
    #[error(transparent)]
    Rand(#[from] rand::Error),

    #[serde(skip)]
    #[error(transparent)]
    ApiSession(#[from] ApiSessionError),
}

impl IntoResponse for AuthnError {
    fn into_response(self) -> Response {
        error::log_prefix_error("error response", &self);

        match &self {
            Self::ClientChallenge |
            Self::InvalidData => (
                StatusCode::BAD_REQUEST,
                body::Json(self),
            ).into_response(),
            Self::DataNotFound |
            Self::UserNotFound |
            Self::SessionNotFound |
            Self::KeyNotFound => (
                StatusCode::NOT_FOUND,
                body::Json(self)
            ).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR
                .into_response(),
        }
    }
}

pub async fn post(
    state: state::SharedState,
    body::Json(api::authn::GetAuthn {
        public_key,
        challenge: client_challenge,
    }): body::Json<api::authn::GetAuthn>
) -> Result<api::authn::AuthnChallenge, AuthnError> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let client = UserClient::retrieve(&transaction, &public_key)
        .await?
        .ok_or(AuthnError::KeyNotFound)?;

    let user_box = {
        let user_dir = state.storage().user_dir(client.users_id);
        let private_key = PrivateKey::load(user_dir.private_key()).await?;

        ChaChaBox::new(&public_key, private_key.secret())
    };

    // attempt to decrypt the provided challenge from the client to verify
    // that this is the peer they expect
    let result = client_challenge.into_data(&user_box).map_err(|_| AuthnError::ClientChallenge)?;

    // generate challenge for the client to verify
    let data = Data::new()?;
    let challenge = data.into_challenge(&user_box)?;

    // create an unauthenticated session for the client
    let options = ApiSessionOptions::new(client.users_id, client.id);
    let session = ApiSession::create(&transaction, options).await?;

    state.security().authn.api.insert(session.token.clone(), data);

    transaction.commit().await?;

    Ok(api::authn::AuthnChallenge {
        result,
        challenge,
        token: session.token,
    })
}

pub async fn patch(
    state: state::SharedState,
    headers: HeaderMap,
    body::Json(api::authn::AuthnResponse {
        result
    }): body::Json<api::authn::AuthnResponse>
) -> Result<StatusCode, AuthnError> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let mut session = match ApiInitiator::from_headers(&transaction, &headers).await {
        Ok(_initiator) => return Ok(StatusCode::OK),
        Err(err) => match err {
            ApiInitiatorError::Unauthenticated(session) => session,
            ApiInitiatorError::NotFound => return Err(AuthnError::SessionNotFound),
            ApiInitiatorError::UserNotFound(_) => return Err(AuthnError::UserNotFound),
            ApiInitiatorError::Expired(_) => return Err(AuthnError::Expired),
            ApiInitiatorError::InvalidAuthorization => return Err(AuthnError::InvalidAuthorization),
            ApiInitiatorError::DbPg(err) => return Err(err.into()),
        }
    };

    let security = state.security();

    if let Some(data) = security.authn.api.get(&session.token) {
        if result == data {
            session.authenticated = true;

            session.update(&transaction).await?;
            security.authn.api.invalidate(&session.token);

            transaction.commit().await?;

            Ok(StatusCode::OK)
        } else {
            Err(AuthnError::InvalidData)
        }
    } else {
        Err(AuthnError::DataNotFound)
    }
}
