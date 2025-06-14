use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::net::{Error, body};
use crate::sec::authn::Initiator;
use crate::sec::otp;
use crate::state::{self, Security};
use crate::user::User;

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum AuthSettings {
    Totp { enabled: bool },
}

#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    kind: Option<AuthKind>,
}

#[derive(Debug, Deserialize)]
pub enum AuthKind {
    Totp,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum GetAuthError {
    MissingKind,
}

impl IntoResponse for GetAuthError {
    fn into_response(self) -> Response {
        match self {
            Self::MissingKind => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
        }
    }
}

pub async fn get(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Query(AuthQuery { kind }): Query<AuthQuery>,
) -> Result<body::Json<AuthSettings>, Error<GetAuthError>> {
    body::assert_html(state.templates(), &headers)?;

    let Some(kind) = kind else {
        return Err(Error::Inner(GetAuthError::MissingKind));
    };

    let conn = state.db().get().await?;

    let result = match kind {
        AuthKind::Totp => AuthSettings::Totp {
            enabled: otp::Totp::exists(&conn, &initiator.user.id).await?,
        },
    };

    Ok(body::Json(result))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum UpdateAuth {
    EnableTotp,
    DisableTotp,
    VerifyTotp { code: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ResultAuth {
    Noop,
    CreatedTotp(ResultTotp),
    DeletedTotp,
    VerifiedTotp,
}

#[derive(Debug, Serialize)]
pub struct ResultTotp {
    algo: otp::Algo,
    step: otp::Step,
    digits: u8,
    secret: String,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum UpdateAuthError {
    InvalidTotpCode,
    TotpNotFound,
    AlreadyExists,
}

impl IntoResponse for UpdateAuthError {
    fn into_response(self) -> Response {
        match self {
            Self::AlreadyExists | Self::InvalidTotpCode => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
            Self::TotpNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn patch(
    state: state::SharedState,
    initiator: Initiator,
    body::Json(action): body::Json<UpdateAuth>,
) -> Result<impl IntoResponse, Error<UpdateAuthError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let result = match action {
        UpdateAuth::EnableTotp => {
            enable_totp(state.security(), &transaction, initiator.user).await?
        }
        UpdateAuth::DisableTotp => {
            disable_totp(state.security(), &transaction, initiator.user).await?
        }
        UpdateAuth::VerifyTotp { code } => {
            verify_totp(state.security(), &transaction, initiator.user, code).await?
        }
    };

    transaction.commit().await?;

    Ok(body::Json(result))
}

pub async fn enable_totp(
    security: &Security,
    conn: &impl db::GenericClient,
    user: User,
) -> Result<ResultAuth, Error<UpdateAuthError>> {
    if otp::Totp::exists(conn, &user.id).await? {
        return Ok(ResultAuth::Noop);
    }

    let totp = match security.vetting.totp.get(&user.id) {
        Some(cached) => cached,
        None => {
            let gen = otp::Totp::generate(user.id)?;

            security.vetting.totp.insert(user.id, gen.clone());

            gen
        }
    };

    let otp::Totp {
        algo,
        step,
        digits,
        secret,
        ..
    } = totp;

    Ok(ResultAuth::CreatedTotp(ResultTotp {
        algo,
        step,
        digits,
        secret: secret.as_base32(),
    }))
}

pub async fn verify_totp(
    security: &Security,
    conn: &impl db::GenericClient,
    user: User,
    code: String,
) -> Result<ResultAuth, Error<UpdateAuthError>> {
    if otp::Totp::exists(conn, &user.id).await? {
        return Ok(ResultAuth::Noop);
    }

    match security.vetting.totp.get(&user.id) {
        Some(record) => {
            if record.verify(code)? {
                if let Err(err) = record.save(conn).await {
                    return Err(match err {
                        otp::TotpError::AlreadyExists => Error::Inner(UpdateAuthError::AlreadyExists),
                        otp::TotpError::Db(err) => Error::from(err)
                    });
                }

                security.vetting.totp.invalidate(&user.id);

                Ok(ResultAuth::VerifiedTotp)
            } else {
                Err(Error::Inner(UpdateAuthError::InvalidTotpCode))
            }
        }
        None => Err(Error::Inner(UpdateAuthError::TotpNotFound)),
    }
}

pub async fn disable_totp(
    security: &Security,
    conn: &impl db::GenericClient,
    user: User,
) -> Result<ResultAuth, Error<UpdateAuthError>> {
    security.vetting.totp.invalidate(&user.id);

    match otp::Totp::retrieve(conn, &user.id).await? {
        Some(record) => match record.delete(conn).await {
            Ok(_) => Ok(ResultAuth::DeletedTotp),
            Err(err) => Err(Error::from(err)),
        },
        None => Ok(ResultAuth::Noop),
    }
}
