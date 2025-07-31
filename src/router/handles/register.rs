use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::InviteToken;
use crate::net::{body, Error};
use crate::sec::authn::session::SessionOptions;
use crate::sec::authn::Session;
use crate::sec::authz::assign_user_role;
use crate::state;
use crate::user::group::assign_user_group;
use crate::user::invite::{Invite, InviteError};
use crate::user::{User, UserBuilder, UserBuilderError};

#[derive(Debug, Deserialize)]
pub struct RegisterBody {
    token: InviteToken,
    username: String,
    password: String,
    confirm: String,
}

// going to try something
#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum RegisterError {
    InviteNotFound,
    InviteUsed,
    InviteExpired,
    InvalidConfirm,
    UsernameExists,
}

impl IntoResponse for RegisterError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::InviteNotFound => StatusCode::NOT_FOUND,
            Self::UsernameExists
            | Self::InvalidConfirm
            | Self::InviteExpired
            | Self::InviteUsed => StatusCode::BAD_REQUEST,
        };

        (status, body::Json(self)).into_response()
    }
}

pub async fn post(
    state: state::SharedState,
    body::Json(body): body::Json<RegisterBody>,
) -> Result<Response, Error<RegisterError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let user = register_user(&transaction, body).await?;

    let mut options = SessionOptions::new(user.id);
    options.authenticated = true;
    options.verified = true;

    let session = Session::create(&transaction, options).await?;
    let session_cookie = session.build_cookie();

    let user_dir = state.storage().user_dir(user.id);

    user_dir.create().await?;

    // do this last since we are making changes to the file system
    let private_key = tj2_lib::sec::pki::PrivateKey::generate()?;

    private_key.save(user_dir.private_key(), false).await?;

    transaction.commit().await?;

    Ok((session_cookie, StatusCode::CREATED).into_response())
}

async fn register_user(
    conn: &impl db::GenericClient,
    RegisterBody {
        token,
        username,
        password,
        confirm,
    }: RegisterBody,
) -> Result<User, Error<RegisterError>> {
    let mut invite = Invite::retrieve(conn, &token)
        .await?
        .ok_or(Error::Inner(RegisterError::InviteNotFound))?;

    if !invite.status.is_pending() {
        return Err(Error::Inner(RegisterError::InviteUsed));
    }

    if invite.is_expired() {
        return Err(Error::Inner(RegisterError::InviteExpired));
    }

    if password != confirm {
        return Err(Error::Inner(RegisterError::InvalidConfirm));
    }

    let builder = UserBuilder::new_password(username, password);
    let user = match builder.build(conn).await {
        Ok(u) => u,
        Err(err) => {
            return Err(match err {
                UserBuilderError::UsernameExists => Error::Inner(RegisterError::UsernameExists),
                UserBuilderError::UidExists => Error::message("user uid collision"),
                UserBuilderError::Db(err) => err.into(),
                UserBuilderError::Argon(err) => err.into(),
            })
        }
    };

    if let Some(role_id) = invite.role_id {
        assign_user_role(conn, role_id, user.id).await?;
    }

    if let Some(groups_id) = invite.groups_id {
        assign_user_group(conn, user.id, groups_id).await?;
    }

    // we have pre-checked that the invite is pending and the user
    // was just created so the id should be valid as well the only
    // thing will be the db
    if let Err(err) = invite.mark_accepted(conn, &user.id).await {
        match err {
            InviteError::Db(err) => return Err(err.into()),
            _ => unreachable!("invite precheck failed {err}"),
        }
    }

    Ok(user)
}
