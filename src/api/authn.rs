use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use tj2_lib::sec::pki::PublicKey;

use crate::net::body;
use crate::sec::authn::session::ApiSessionToken;
use crate::sec::pki::{Challenge, Data};

#[derive(Debug, Serialize, Deserialize)]
pub struct GetAuthn {
    /// public key sent by client to authenticate for user
    pub public_key: PublicKey,

    /// encrypted data sent by client to be decrypted by peer and sent back,
    /// includes nonce at the start of the bytes
    pub challenge: Challenge,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthnChallenge {
    /// data decrypted by peer
    pub result: Data,

    /// encrypted data set by peer to be decrypted by client and sent back,
    /// includes nonce at the start of the bytes
    pub challenge: Challenge,

    /// associated auth token to use for subsequent requests.
    pub token: ApiSessionToken,
}

impl IntoResponse for AuthnChallenge {
    fn into_response(self) -> Response {
        (StatusCode::OK, body::Json(self)).into_response()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthnResponse {
    /// data decrypted by client
    pub result: Data,
}
