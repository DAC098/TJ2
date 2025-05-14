use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::InviteToken;
use crate::error::{self, Context};
use crate::router::{body, macros};
use crate::sec::authn::Session;
use crate::sec::authn::session::SessionOptions;
use crate::sec;
use crate::state;
use crate::user::{User, UserBuilder, UserBuilderError};
use crate::user::invite::{Invite, InviteError};

pub async fn get(
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

pub async fn post(
    state: state::SharedState,
    body::Json(body): body::Json<RegisterBody>
) -> Result<Response, RegisterError> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let user = register_user(&transaction, body).await?;

    let mut options = SessionOptions::new(user.id);
    options.authenticated = true;
    options.verified = true;

    let session = Session::create(&transaction, options)
        .await
        .context("failed to create session record")?;
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
