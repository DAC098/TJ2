use axum::debug_handler;
use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::net::{body, Error};
use crate::sec::authn::Initiator;
use crate::sec;
use crate::sec::mfa::{otp, create_recovery, delete_recovery};
use crate::state::{self, Security};

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum AuthSettings {
    Totp { enabled: bool },
    Recovery {
        used_on: Vec<Option<DateTime<Utc>>>,
    },
}

#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    kind: Option<AuthKind>,
}

#[derive(Debug, Deserialize)]
pub enum AuthKind {
    Totp,
    Recovery,
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
        AuthKind::Recovery => {
            let stream = conn.query_raw(
                "select used_on from authn_recovery where users_id = $1 order by used_on",
                &[&initiator.user.id]
            ).await?;

            futures::pin_mut!(stream);

            let mut used_on = Vec::with_capacity(5);

            while let Some(maybe) = stream.next().await {
                let row = maybe?;

                used_on.push(row.get(0));
            }

            AuthSettings::Recovery { used_on }
        }
    };

    Ok(body::Json(result))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum UpdateAuth {
    // MFA totp
    EnableTotp,
    DisableTotp,
    VerifyTotp { code: String },

    // MFA Recovery
    EnableRecovery,
    DisableRecovery,
    ResetRecovery,

    // Password
    UpdatePassword {
        current: String,
        updated: String,
        confirm: String,
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ResultAuth {
    Noop,

    // MFA Totp
    EnabledTotp(ResultTotp),
    DisabledTotp,
    VerifiedTotp,

    // MFA Recovery
    EnabledRecovery {
        codes: Vec<String>,
    },
    DisabledRecovery,
    ResetRecovery {
        codes: Vec<String>,
    },

    // Password
    UpdatedPassword,
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
    TotpAlreadyExists,

    NoMFAEnabled,
    RecoveryExists,

    InvalidPassword,
    InvalidConfirm,
}

impl IntoResponse for UpdateAuthError {
    fn into_response(self) -> Response {
        match self {
            Self::InvalidTotpCode => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::TotpNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::TotpAlreadyExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),

            Self::NoMFAEnabled => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::RecoveryExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),

            Self::InvalidConfirm => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
            Self::InvalidPassword => (StatusCode::FORBIDDEN, body::Json(self)).into_response(),
        }
    }
}

#[debug_handler]
pub async fn patch(
    state: state::SharedState,
    initiator: Initiator,
    body::Json(action): body::Json<UpdateAuth>,
) -> Result<impl IntoResponse, Error<UpdateAuthError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let result = match action {
        UpdateAuth::EnableTotp => {
            enable_totp(state.security(), &transaction, initiator).await?
        }
        UpdateAuth::DisableTotp => {
            disable_totp(state.security(), &transaction, initiator).await?
        }
        UpdateAuth::VerifyTotp { code } => {
            verify_totp(state.security(), &transaction, initiator, code).await?
        }
        UpdateAuth::EnableRecovery => {
            enable_recovery(&transaction, initiator).await?
        }
        UpdateAuth::DisableRecovery => {
            disable_recovery(&transaction, initiator).await?
        }
        UpdateAuth::ResetRecovery => {
            reset_recovery(&transaction, initiator).await?
        }
        UpdateAuth::UpdatePassword { current, updated, confirm } => {
            update_password(&transaction, initiator, current, updated, confirm).await?
        }
    };

    transaction.commit().await?;

    Ok(body::Json(result))
}

async fn enable_totp(
    security: &Security,
    conn: &impl db::GenericClient,
    initiator: Initiator,
) -> Result<ResultAuth, Error<UpdateAuthError>> {
    if otp::Totp::exists(conn, &initiator.user.id).await? {
        return Ok(ResultAuth::Noop);
    }

    let totp = match security.vetting.totp.get(&initiator.user.id) {
        Some(cached) => cached,
        None => {
            let gen = otp::Totp::generate(initiator.user.id)?;

            security.vetting.totp.insert(initiator.user.id, gen.clone());

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

    Ok(ResultAuth::EnabledTotp(ResultTotp {
        algo,
        step,
        digits,
        secret: secret.as_base32(),
    }))
}

async fn verify_totp(
    security: &Security,
    conn: &impl db::GenericClient,
    initiator: Initiator,
    code: String,
) -> Result<ResultAuth, Error<UpdateAuthError>> {
    if otp::Totp::exists(conn, &initiator.user.id).await? {
        return Ok(ResultAuth::Noop);
    }

    match security.vetting.totp.get(&initiator.user.id) {
        Some(record) => {
            if record.verify(code)? {
                if let Err(err) = record.save(conn).await {
                    return Err(match err {
                        otp::TotpError::AlreadyExists => {
                            Error::Inner(UpdateAuthError::TotpAlreadyExists)
                        }
                        otp::TotpError::Db(err) => Error::from(err),
                    });
                }

                security.vetting.totp.invalidate(&initiator.user.id);

                Ok(ResultAuth::VerifiedTotp)
            } else {
                Err(Error::Inner(UpdateAuthError::InvalidTotpCode))
            }
        }
        None => Err(Error::Inner(UpdateAuthError::TotpNotFound)),
    }
}

async fn disable_totp(
    security: &Security,
    conn: &impl db::GenericClient,
    initiator: Initiator,
) -> Result<ResultAuth, Error<UpdateAuthError>> {
    security.vetting.totp.invalidate(&initiator.user.id);

    match otp::Totp::retrieve(conn, &initiator.user.id).await? {
        Some(record) => match record.delete(conn).await {
            Ok(_) => Ok(ResultAuth::DisabledTotp),
            Err(err) => Err(Error::from(err)),
        },
        None => Ok(ResultAuth::Noop),
    }
}

async fn enable_recovery(
    conn: &impl db::GenericClient,
    initiator: Initiator,
) -> Result<ResultAuth, Error<UpdateAuthError>> {
    if !otp::Totp::exists(conn, &initiator.user.id).await? {
        return Err(Error::Inner(UpdateAuthError::NoMFAEnabled));
    }

    let result = conn.execute(
        "select used_on from authn_recovery where users_id = $1",
        &[&initiator.user.id],
    ).await?;

    if result != 0 {
        return Err(Error::Inner(UpdateAuthError::RecoveryExists));
    }

    let codes = create_recovery(conn, &initiator.user.id).await?;

    Ok(ResultAuth::EnabledRecovery { codes })
}

async fn disable_recovery(
    conn: &impl db::GenericClient,
    initiator: Initiator,
) -> Result<ResultAuth, Error<UpdateAuthError>> {
    if !otp::Totp::exists(conn, &initiator.user.id).await? {
        return Err(Error::Inner(UpdateAuthError::NoMFAEnabled));
    }

    let amount = delete_recovery(conn, &initiator.user.id).await?;

    if amount == 0 {
        Ok(ResultAuth::Noop)
    } else {
        Ok(ResultAuth::DisabledRecovery)
    }
}

async fn reset_recovery(
    conn: &impl db::GenericClient,
    initiator: Initiator,
) -> Result<ResultAuth, Error<UpdateAuthError>> {
    if !otp::Totp::exists(conn, &initiator.user.id).await? {
        return Err(Error::Inner(UpdateAuthError::NoMFAEnabled));
    }

    delete_recovery(conn, &initiator.user.id).await?;

    let codes = create_recovery(conn, &initiator.user.id).await?;

    Ok(ResultAuth::ResetRecovery { codes })
}

async fn update_password(
    conn: &impl db::GenericClient,
    mut initiator: Initiator,
    current: String,
    updated: String,
    confirm: String,
) -> Result<ResultAuth, Error<UpdateAuthError>> {
    if updated != confirm {
        return Err(Error::Inner(UpdateAuthError::InvalidConfirm));
    }

    if !sec::password::verify(&initiator.user.password, &current)? {
        return Err(Error::Inner(UpdateAuthError::InvalidPassword));
    }

    initiator.user.password = sec::password::create(&updated)?;

    initiator.user.update(conn).await?;

    conn.execute(
        "delete from authn_sessions where users_id = $1 and token != $2",
        &[&initiator.user.id, &initiator.session.token]
    ).await?;

    Ok(ResultAuth::UpdatedPassword)
}
