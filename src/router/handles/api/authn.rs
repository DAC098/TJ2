use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use crypto_box::ChaChaBox;
use serde::{Deserialize, Serialize};
use tj2_lib::sec::pki::PrivateKey;

use crate::api;
use crate::net::{body, Error};
use crate::sec::authn::session::ApiSessionOptions;
use crate::sec::authn::{ApiInitiator, ApiInitiatorError, ApiSession};
use crate::sec::pki::Data;
use crate::state;
use crate::user::client::UserClient;

#[derive(Debug, strum::Display, Serialize, Deserialize)]
pub enum AuthnError {
    KeyNotFound,
    ClientChallenge,
    InvalidAuthorization,
    SessionNotFound,
    UserNotFound,
    DataNotFound,
    Expired,
    InvalidData,
}

impl IntoResponse for AuthnError {
    fn into_response(self) -> Response {
        match self {
            Self::ClientChallenge | Self::InvalidData | Self::InvalidAuthorization => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
            Self::Expired => (StatusCode::UNAUTHORIZED, body::Json(self)).into_response(),
            Self::DataNotFound | Self::UserNotFound | Self::SessionNotFound | Self::KeyNotFound => {
                (StatusCode::NOT_FOUND, body::Json(self)).into_response()
            }
        }
    }
}

pub async fn post(
    state: state::SharedState,
    body::Json(api::authn::GetAuthn {
        public_key,
        challenge: client_challenge,
    }): body::Json<api::authn::GetAuthn>,
) -> Result<api::authn::AuthnChallenge, Error<AuthnError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let client = UserClient::retrieve(&transaction, &public_key)
        .await?
        .ok_or(Error::Inner(AuthnError::KeyNotFound))?;

    let user_box = {
        let user_dir = state.storage().user_dir(client.users_id);
        let private_key = PrivateKey::load(user_dir.private_key()).await?;

        ChaChaBox::new(&public_key, private_key.secret())
    };

    // attempt to decrypt the provided challenge from the client to verify
    // that this is the peer they expect
    let result = client_challenge
        .into_data(&user_box)
        .map_err(|_| Error::Inner(AuthnError::ClientChallenge))?;

    // generate challenge for the client to verify
    let data = Data::new()?;
    let challenge = data.into_challenge(&user_box)?;

    // create an unauthenticated session for the client
    let options = ApiSessionOptions::new(client.users_id, client.id);
    let session = ApiSession::create(&transaction, options).await?;

    state
        .security()
        .authn
        .api
        .insert(session.token.clone(), data);

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
    body::Json(api::authn::AuthnResponse { result }): body::Json<api::authn::AuthnResponse>,
) -> Result<StatusCode, Error<AuthnError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let mut session = match ApiInitiator::from_headers(&transaction, &headers).await {
        Ok(_initiator) => return Ok(StatusCode::OK),
        Err(err) => match err {
            ApiInitiatorError::Unauthenticated(session) => session,
            ApiInitiatorError::NotFound => return Err(Error::Inner(AuthnError::SessionNotFound)),
            ApiInitiatorError::UserNotFound(_) => {
                return Err(Error::Inner(AuthnError::UserNotFound))
            }
            ApiInitiatorError::Expired(_) => return Err(Error::Inner(AuthnError::Expired)),
            ApiInitiatorError::InvalidAuthorization => {
                return Err(Error::Inner(AuthnError::InvalidAuthorization))
            }
            ApiInitiatorError::DbPg(err) => return Err(err.into()),
        },
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
            Err(Error::Inner(AuthnError::InvalidData))
        }
    } else {
        Err(Error::Inner(AuthnError::DataNotFound))
    }
}
