use argon2::{Argon2, PasswordVerifier};
use argon2::password_hash::PasswordHash;
use axum::extract::Query;
use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::error::{self, Context};
use crate::header::{Location, is_accepting_html};
use crate::router::body;
use crate::sec::authn::{Session, Initiator, InitiatorError};
use crate::sec::authn::session::SessionOptions;
use crate::state;
use crate::user;

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum LoginResult {
    Success,
    Failed(LoginFailed)
}

#[derive(Debug, Serialize)]
pub enum LoginFailed {
    UsernameNotFound,
    InvalidPassword,
}

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    prev: Option<String>
}

impl LoginQuery {
    fn get_prev(&self) -> Option<String> {
        if let Some(prev) = &self.prev {
            match urlencoding::decode(prev) {
                Ok(cow) => Some(cow.into_owned()),
                Err(err) => {
                    tracing::warn!("failed to decode login prev query: {err}");

                    None
                }
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize)]
pub enum LoginStatus {
    Active,
    Inactive
}

#[derive(Debug, Serialize)]
pub struct LoginCheck {
    status: LoginStatus
}

pub async fn get(
    state: state::SharedState,
    Query(query): Query<LoginQuery>,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    let conn = state.db()
        .get()
        .await
        .context("failed to retrieve database connection")?;

    let result = Initiator::from_headers(&conn, &headers).await;

    let Ok(is_html) = is_accepting_html(&headers) else {
        return Ok((
            StatusCode::BAD_REQUEST,
            "invalid characters in accept header"
        ).into_response());
    };

    if is_html {
        if let Err(err) = result {
            error::log_prefix_error(
                "error when retrieving session id",
                &err
            );

            Ok((
                Session::clear_cookie(),
                body::SpaPage::new(state.templates())?
            ).into_response())
        } else {
            Ok(Location::to(
                query.get_prev().unwrap_or("/".to_owned())
            ).into_response())
        }
    } else {
        if let Err(err) = result {
            match err {
                InitiatorError::DbPg(err) => Err(error::Error::context_source(
                    "database error when retrieving session",
                    err
                )),
                err => {
                    error::log_prefix_error(
                        "error when retrieving session id",
                        &err
                    );

                    Ok(body::Json(LoginCheck {
                        status: LoginStatus::Inactive
                    }).into_response())
                }
            }
        } else {
            Ok(body::Json(LoginCheck {
                status: LoginStatus::Active
            }).into_response())
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

pub async fn post(
    state: state::SharedState,
    body::Json(login): body::Json<LoginRequest>,
) -> Result<Response, error::Error> {
    let mut conn = state.db()
        .get()
        .await
        .context("failed to retrieve database connection")?;

    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let maybe_user = user::User::retrieve_username(&transaction, &login.username)
        .await
        .context("database error when searching for login username")?;

    let Some(user) = maybe_user else {
        return Ok((
            StatusCode::NOT_FOUND,
            body::Json(LoginResult::Failed(LoginFailed::UsernameNotFound))
        ).into_response());
    };

    let argon_config = Argon2::default();
    let parsed_hash = match PasswordHash::new(&user.password) {
        Ok(hash) => hash,
        Err(err) => {
            tracing::debug!("argon2 PasswordHash error: {err:#?}");

            return Err(error::Error::context("failed to create argon2 password hash"));
        }
    };

    if let Err(err) = argon_config.verify_password(login.password.as_bytes(), &parsed_hash) {
        tracing::debug!("verify_password failed: {err:#?}");

        return Ok((
            StatusCode::FORBIDDEN,
            body::Json(LoginResult::Failed(LoginFailed::InvalidPassword))
        ).into_response());
    }

    let mut options = SessionOptions::new(user.id);
    options.authenticated = true;
    options.verified = true;

    let session = Session::create(&transaction, options)
        .await
        .context("failed to create session for login")?;

    let session_cookie = session.build_cookie();

    transaction.commit()
        .await
        .context("failed to commit transaction for login")?;

    Ok((
        session_cookie,
        body::Json(LoginResult::Success)
    ).into_response())
}
