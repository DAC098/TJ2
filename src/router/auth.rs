use argon2::{Argon2, PasswordVerifier};
use argon2::password_hash::PasswordHash;
use axum::extract::Query;
use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::InviteToken;
use crate::error::{self, Context};
use crate::header::{Location, is_accepting_html};
use crate::router::{body, macros};
use crate::sec::authn::{Session, Initiator, InitiatorError};
use crate::sec::authn::session::SessionOptions;
use crate::sec;
use crate::state;
use crate::user::{self, User, UserBuilder, UserBuilderError};
use crate::user::invite::{Invite, InviteError};

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

pub async fn login(
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

pub async fn request_login(
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

pub async fn request_logout(
    state: state::SharedState,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    let mut conn = state.db()
        .get()
        .await
        .context("failed to retrieve database connection")?;

    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    match Initiator::from_headers(&transaction, &headers).await {
        Ok(initiator) => {
            initiator.session.delete(&transaction)
                .await
                .context("failed to delete session from database")?;
        }
        Err(err) => match err{
            InitiatorError::UserNotFound(session) |
            InitiatorError::Unauthenticated(session) |
            InitiatorError::Unverified(session) |
            InitiatorError::SessionExpired(session) => {
                session.delete(&transaction)
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

pub async fn get_register(
    state: state::SharedState,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    Ok(body::Json("okay").into_response())
}

#[derive(Debug, Deserialize)]
pub struct RegisterBody {
    token: InviteToken,
    username: String,
    password: String,
    confirm: String,
}

// going to try something
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "type")]
pub enum RegisterError {
    #[error("the requested invite was not found")]
    InviteNotFound,

    #[error("the requested invite has already been used")]
    InviteUsed,

    #[error("the requested invite has expired")]
    InviteExpired,

    #[error("the confirm does not equal password")]
    InvalidConfirm,

    #[error("the specified username already exists")]
    UsernameExists,

    #[serde(skip)]
    #[error(transparent)]
    Db(#[from] db::PgError),

    #[serde(skip)]
    #[error(transparent)]
    DbPool(#[from] db::PoolError),

    #[serde(skip)]
    #[error(transparent)]
    Argon(#[from] sec::password::HashError),

    #[serde(skip)]
    #[error(transparent)]
    Error(#[from] error::Error),

    #[serde(skip)]
    #[error(transparent)]
    Io(#[from] std::io::Error)
}

impl IntoResponse for RegisterError {
    fn into_response(self) -> Response {
        error::log_error(&self);

        let status = match &self {
            Self::InviteNotFound => StatusCode::NOT_FOUND,
            Self::UsernameExists |
            Self::InvalidConfirm |
            Self::InviteExpired |
            Self::InviteUsed => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, body::Json(self)).into_response()
    }
}

pub async fn register(
    state: state::SharedState,
    body::Json(body): body::Json<RegisterBody>
) -> Result<Response, RegisterError> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let user = register_user(&transaction, body).await?;

    let mut options = SessionOptions::new(user.id);
    options.authenticated = true;
    options.verified = true;

    let session = Session::create(&transaction, options).await?;
    let session_cookie = session.build_cookie();

    let user_dir = state.storage()
        .user_dir(user.id);

    user_dir.create().await?;

    // do this last since we are making changes to the file system
    let private_key = tj2_lib::sec::pki::PrivateKey::generate()
        .context("failed to generate private key")?;

    private_key.save(user_dir.private_key(), false)
        .await
        .context("failed to save private key")?;

    transaction.commit().await?;

    Ok((
        session_cookie,
        StatusCode::CREATED
    ).into_response())
}

async fn register_user(
    conn: &impl db::GenericClient,
    RegisterBody {
        token,
        username,
        password,
        confirm,
    }: RegisterBody
) -> Result<User, RegisterError> {
    let mut invite = Invite::retrieve(conn, &token)
        .await?
        .ok_or(RegisterError::InviteNotFound)?;

    if !invite.status.is_pending() {
        return Err(RegisterError::InviteUsed);
    }

    if invite.is_expired() {
        return Err(RegisterError::InviteExpired);
    }

    if password != confirm {
        return Err(RegisterError::InvalidConfirm);
    }

    let builder = match UserBuilder::new_password(username, password) {
        Ok(b) => b,
        Err(err) => match err {
            UserBuilderError::Argon(argon_err) => return Err(argon_err.into()),
            _ => unreachable!()
        }
    };
    let user = match builder.build(conn).await {
        Ok(u) => u,
        Err(err) => match err {
            UserBuilderError::UsernameExists =>
                return Err(RegisterError::UsernameExists),
            UserBuilderError::UidExists =>
                return Err(error::Error::context("user uid collision").into()),
            UserBuilderError::Db(db_err) =>
                return Err(db_err.into()),
            _ => unreachable!(),
        }
    };

    // we have pre-checked that the invite is pending and the user
    // was just created so the id should be valid as well the only
    // thing will be the db
    if let Err(err) = invite.mark_accepted(conn, &user.id).await {
        match err {
            InviteError::Db(db_err) => return Err(db_err.into()),
            _ => unreachable!("invite precheck failed {err}"),
        }
    }

    Ok(user)
}
