use axum::extract::Query;
use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::{self, Context};
use crate::header::{Location, is_accepting_html};
use crate::router::body;
use crate::sec;
use crate::sec::authn::{Session, Initiator, InitiatorError};
use crate::sec::authn::session::SessionOptions;
use crate::sec::otp;
use crate::state;
use crate::user;

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

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum LoginResult {
    Success,
    Verify
}

#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "type")]
pub enum LoginError {
    #[error("the specified username was not found")]
    UsernameNotFound,

    #[error("invalid password provided")]
    InvalidPassword,

    #[error("user has already been authenticated")]
    AlreadyAuthenticated,

    // will have to think about how to handle this later on
    #[error("invalid session id")]
    InvalidSession,

    #[serde(skip)]
    #[error(transparent)]
    Db(#[from] db::PgError),

    #[serde(skip)]
    #[error(transparent)]
    DbPool(#[from] db::PoolError),

    #[serde(skip)]
    #[error(transparent)]
    Hash(#[from] sec::password::HashError),

    #[serde(skip)]
    #[error(transparent)]
    Error(#[from] error::Error),
}

impl IntoResponse for LoginError {
    fn into_response(self) -> Response {
        error::log_prefix_error("response error", &self);

        match self {
            Self::AlreadyAuthenticated => (
                StatusCode::BAD_REQUEST,
                body::Json(self)
            ).into_response(),
            Self::UsernameNotFound => (
                StatusCode::NOT_FOUND,
                body::Json(self)
            ).into_response(),
            Self::InvalidPassword => (
                StatusCode::FORBIDDEN,
                body::Json(self)
            ).into_response(),
            _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

pub async fn post(
    state: state::SharedState,
    headers: HeaderMap,
    body::Json(login): body::Json<LoginRequest>,
) -> Result<impl IntoResponse, LoginError> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let result = Initiator::from_headers(&transaction, &headers).await;

    tracing::debug!("initiator result: {result:#?}");

    match result {
        Ok(_) => return Err(LoginError::AlreadyAuthenticated),
        Err(err) => match err {
            InitiatorError::SessionIdNotFound => {}
            InitiatorError::Unverified(session) => {
                session.delete(&transaction).await?;
            }
            InitiatorError::DbPg(err) => return Err(LoginError::Db(err)),
            _ => return Err(LoginError::InvalidSession),
        }
    }

    let user = user::User::retrieve(&transaction, &login.username)
        .await?
        .ok_or(LoginError::UsernameNotFound)?;

    if !sec::password::verify(&user.password, &login.password)? {
        return Err(LoginError::InvalidPassword);
    }

    let mut options = SessionOptions::new(user.id);
    options.authenticated = true;

    let result = if otp::Totp::exists(&transaction, &user.id).await? {
        options.verified = false;

        LoginResult::Verify
    } else {
        options.verified = true;

        LoginResult::Success
    };

    let session = Session::create(&transaction, options).await?;
    let session_cookie = session.build_cookie();

    transaction.commit().await?;

    Ok((
        session_cookie,
        body::Json(result)
    ).into_response())
}
