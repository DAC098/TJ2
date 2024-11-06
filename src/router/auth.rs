use argon2::{Argon2, PasswordVerifier};
use argon2::password_hash::PasswordHash;
use axum::extract::Query;
use axum::http::{StatusCode, HeaderMap};
use axum::body::Body;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::error::{self, Context};
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

pub async fn login(
    state: state::SharedState,
    Query(query): Query<LoginQuery>,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    let conn = state.db_pg()
        .get()
        .await
        .context("failed to retrieve database connection")?;

    let result = Initiator::from_headers_pg(&conn, &headers)
        .await;

    match result {
        Ok(_) => {
            tracing::debug!("session for initiator is valid");

            let location = query.get_prev()
                .unwrap_or("/".to_owned());

            Response::builder()
                .status(StatusCode::FOUND)
                .header("location", location)
                .body(Body::empty())
                .context("failed to create redirect response")
        }
        Err(err) => match err {
            InitiatorError::SessionIdNotFound => {
                tracing::debug!("session id not found");

                Ok(body::SpaPage::new(state.templates())?.into_response())
            }
            InitiatorError::SessionNotFound |
            InitiatorError::UserNotFound(_) |
            InitiatorError::Unauthenticated(_) |
            InitiatorError::Unverified(_) |
            InitiatorError::SessionExpired(_) => {
                tracing::debug!("problem with session");

                Ok((
                    Session::clear_cookie(),
                    body::SpaPage::new(state.templates())?
                ).into_response())
            }
            InitiatorError::HeaderStr(err) => {
                Err(error::Error::context_source(
                    "error when parsing cookie headers",
                    err
                ))
            }
            InitiatorError::Token(err) => {
                Err(error::Error::context_source(
                    "invalid session token",
                    err
                ))
            }
            InitiatorError::Db(err) => {
                Err(error::Error::context_source(
                    "database error when retrieving session",
                    err
                ))
            }
            InitiatorError::DbPg(err) => {
                Err(error::Error::context_source(
                    "database error when retrieving session",
                    err
                ))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

pub async fn request_login(
    state: state::SharedState,
    body::Json(login): body::Json<LoginRequest>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_pg()
        .get()
        .await
        .context("failed to retrieve database connection")?;

    let transaction = conn.transaction()
        .await
        .context("failedto create transaction")?;

    tracing::debug!("login recieved: {login:#?}");

    let maybe_user = user::User::retrieve_username_pg(&transaction, &login.username)
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

    let session = Session::create_pg(&transaction, options)
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

pub async fn request_logout(
    state: state::SharedState,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    let mut conn = state.db_pg()
        .get()
        .await
        .context("failed to retrieve database connection")?;

    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    match Initiator::from_headers_pg(&transaction, &headers).await {
        Ok(initiator) => {
            initiator.session.delete_pg(&transaction)
                .await
                .context("failed to delete session from database")?;
        }
        Err(err) => match err{
            InitiatorError::UserNotFound(session) |
            InitiatorError::Unauthenticated(session) |
            InitiatorError::Unverified(session) |
            InitiatorError::SessionExpired(session) => {
                session.delete_pg(&transaction)
                    .await
                    .context("failed to delete session from database")?;
            }
            InitiatorError::HeaderStr(_err) => {}
            InitiatorError::Token(_err) => {}
            InitiatorError::DbPg(err) =>
                return Err(error::Error::context_source(
                    "database error when retrieving session",
                    err
                )),
            _ => {}
        }
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok((
        StatusCode::OK,
        Session::clear_cookie()
    ).into_response())
}
