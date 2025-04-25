use axum::http::{StatusCode, HeaderMap};
use axum::response::{Response, IntoResponse};
use serde::{Serialize, Deserialize};

use crate::db;
use crate::error;
use crate::router::{body, macros};
use crate::state;
use crate::sec::authn::Initiator;
use crate::sec::otp;
use crate::user::User;

pub async fn get(
    state: state::SharedState,
    _initiator: Initiator,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    Ok(body::Json("ok").into_response())
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum UpdateAuth {
    Totp(UpdateTotp),
}

#[derive(Debug, Deserialize)]
pub struct UpdateTotp {
    enable: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ResultAuth {
    Noop,
    CreatedTotp(ResultTotp),
    DeletedTotp,
}

#[derive(Debug, Serialize)]
pub struct ResultTotp {
    algo: otp::Algo,
    step: otp::Step,
    digits: u8,
    secret: String,
}

#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "type")]
pub enum UpdateAuthError {
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
    Rand(#[from] rand::Error),
}

impl IntoResponse for UpdateAuthError {
    fn into_response(self) -> Response {
        let status = match self {
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, body::Json(self)).into_response()
    }
}

pub async fn patch(
    state: state::SharedState,
    initiator: Initiator,
    body::Json(action): body::Json<UpdateAuth>
) -> Result<impl IntoResponse, UpdateAuthError> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let result = match action {
        UpdateAuth::Totp(options) => update_totp(
            &transaction, initiator.user, options
        ).await?,
    };

    transaction.commit().await?;

    Ok(body::Json(result))
}

pub async fn update_totp(
    conn: &impl db::GenericClient,
    user: User,
    UpdateTotp {
        enable
    }: UpdateTotp
) -> Result<ResultAuth, UpdateAuthError> {
    let maybe_exists = otp::Totp::retrieve(conn, &user.id).await?;

    match (enable, maybe_exists) {
        (true, Some(_)) => Ok(ResultAuth::Noop),
        (true, None) => match otp::Totp::create(conn, &user.id).await {
            Ok(otp::Totp { algo, step, digits, secret, .. }) => Ok(ResultAuth::CreatedTotp(ResultTotp {
                algo,
                step,
                digits,
                secret: secret.as_base32(),
            })),
            Err(err) => match err {
                otp::TotpError::Db(err) => Err(UpdateAuthError::Db(err)),
                otp::TotpError::Rand(err) => Err(UpdateAuthError::Rand(err)),
                otp::TotpError::AlreadyExists => Ok(ResultAuth::Noop),
            }
        },
        (false, Some(record)) => match record.delete(conn).await {
            Ok(_) => Ok(ResultAuth::DeletedTotp),
            Err(err) => match err {
                otp::TotpError::Db(err) => Err(UpdateAuthError::Db(err)),
                _ => unreachable!(),
            }
        },
        (false, None) => Ok(ResultAuth::Noop),
    }
}
