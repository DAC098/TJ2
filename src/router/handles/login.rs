use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::db::ids::UserId;
use crate::error;
use crate::net::header::{is_accepting_html, Location};
use crate::net::{body, Error as NetError};
use crate::sec;
use crate::sec::authn::session::SessionOptions;
use crate::sec::authn::{Initiator, InitiatorError, Session};
use crate::sec::mfa::otp;
use crate::state;
use crate::user;

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    prev: Option<String>,
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
    Inactive,
}

#[derive(Debug, Serialize)]
pub struct LoginCheck {
    status: LoginStatus,
}

impl IntoResponse for LoginCheck {
    fn into_response(self) -> Response {
        (StatusCode::OK, body::Json(self)).into_response()
    }
}

pub async fn get(
    state: state::SharedState,
    headers: HeaderMap,
    Query(query): Query<LoginQuery>,
) -> Result<Response, NetError> {
    let conn = state.db().get().await?;

    let result = Initiator::from_headers(&conn, &headers).await;

    if is_accepting_html(&headers)? {
        if let Err(err) = result {
            error::log_prefix_error("error when retrieving session id", &err);

            Ok((
                Session::clear_cookie(),
                body::SpaPage::new(state.templates())?,
            )
                .into_response())
        } else {
            Ok(Location::to(query.get_prev().unwrap_or("/".to_owned())).into_response())
        }
    } else {
        if let Err(err) = result {
            match err {
                InitiatorError::DbPg(err) => Err(err.into()),
                err => {
                    error::log_prefix_error("error when retrieving session id", &err);

                    Ok(LoginCheck {
                        status: LoginStatus::Inactive,
                    }
                    .into_response())
                }
            }
        } else {
            Ok(LoginCheck {
                status: LoginStatus::Active,
            }
            .into_response())
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
    Success { id: UserId, username: String },
    Verify,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum LoginError {
    UsernameNotFound,
    InvalidPassword,
    AlreadyAuthenticated,
    InvalidSession,
}

impl IntoResponse for LoginError {
    fn into_response(self) -> Response {
        match self {
            Self::AlreadyAuthenticated => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
            Self::UsernameNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::InvalidPassword => (StatusCode::FORBIDDEN, body::Json(self)).into_response(),
            Self::InvalidSession => (StatusCode::FORBIDDEN, body::Json(self)).into_response(),
        }
    }
}

pub async fn post(
    state: state::SharedState,
    headers: HeaderMap,
    body::Json(login): body::Json<LoginRequest>,
) -> Result<impl IntoResponse, NetError<LoginError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let result = Initiator::from_headers(&transaction, &headers).await;

    match result {
        Ok(_) => return Err(NetError::Inner(LoginError::AlreadyAuthenticated)),
        Err(err) => match err {
            InitiatorError::SessionIdNotFound => {}
            InitiatorError::Unverified(session) => {
                session.delete(&transaction).await?;
            }
            InitiatorError::DbPg(err) => return Err(err.into()),
            _ => return Err(NetError::Inner(LoginError::InvalidSession)),
        },
    }

    let user = user::User::retrieve(&transaction, &login.username)
        .await?
        .ok_or(NetError::Inner(LoginError::UsernameNotFound))?;

    if !sec::password::verify(&user.password, &login.password)? {
        return Err(NetError::Inner(LoginError::InvalidPassword));
    }

    let mut options = SessionOptions::new(user.id);
    options.authenticated = true;

    let result = if otp::Totp::exists(&transaction, &user.id).await? {
        options.verified = false;

        LoginResult::Verify
    } else {
        options.verified = true;

        LoginResult::Success {
            id: user.id,
            username: user.username,
        }
    };

    let session = Session::create(&transaction, options).await?;
    let session_cookie = session.build_cookie();

    transaction.commit().await?;

    Ok((session_cookie, body::Json(result)).into_response())
}
