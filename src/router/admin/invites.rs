use axum::extract::Path;
use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};
use chrono::{Utc, DateTime};
use futures::StreamExt;
use serde::{Serialize, Deserialize};

use crate::db;
use crate::db::ids::InviteToken;
use crate::error::{self, Context};
use crate::router::{body, macros};
use crate::sec::authn::Initiator;
use crate::state;
use crate::user::{Invite, InviteStatus};

#[derive(Debug, Serialize)]
pub struct PartialInvite {
    token: InviteToken,
    name: String,
    issued_on: DateTime<Utc>,
    expires_on: Option<DateTime<Utc>>,
    status: InviteStatus,
}

pub async fn search_invites(
    state: state::SharedState,
    _initiator: Initiator,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let conn = state.db_conn().await?;

    // check if the user has permission to view invites

    let params: db::ParamsArray<'_, 0> = [];
    let stream = conn.query_raw(
        "\
        select user_invites.token, \
               user_invites.name, \
               user_invites.issued_on, \
               user_invites.expires_on, \
               user_invites.status \
        from user_invites",
        params
    )
        .await
        .context("failed to query user invites")?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let record = try_record.context("failed to retrieve record")?;

        rtn.push(PartialInvite {
            token: record.get(0),
            name: record.get(1),
            issued_on: record.get(2),
            expires_on: record.get(3),
            status: record.get(4),
        });
    }

    Ok((
        StatusCode::OK,
        body::Json(rtn)
    ).into_response())
}

#[derive(Debug, Deserialize)]
pub struct InvitePath {
    token: InviteToken
}

pub async fn new_invite(
    state: state::SharedState,
    _initiator: Initiator,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    Ok(body::Json("Ok").into_response())
}

#[derive(Debug, Serialize)]
pub struct InviteForm {
    token: InviteToken,
    name: String,
    issued_on: DateTime<Utc>,
    expires_on: InviteExpires,
    status: InviteStatus,
}

#[derive(Debug, Serialize)]
pub struct InviteExpires {
    enabled: bool,
    date: DateTime<Utc>,
}

pub async fn retrieve_invite(
    state: state::SharedState,
    _initiator: Initiator,
    headers: HeaderMap,
    Path(InvitePath {
        token,
    }): Path<InvitePath>,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let conn = state.db_conn().await?;

    // check if the user has permission to view invites

    let maybe = Invite::retrieve(&conn, &token)
        .await
        .context("failed to retrieve user invite")?;

    let Some(Invite {
        name,
        issued_on,
        expires_on,
        status,
        ..
    }) = maybe else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    Ok((
        StatusCode::OK,
        body::Json(InviteForm {
            token,
            name,
            issued_on,
            expires_on: InviteExpires {
                enabled: expires_on.is_some(),
                date: expires_on.unwrap_or_default(),
            },
            status,
        })
    ).into_response())
}

#[derive(Debug, Deserialize)]
pub struct NewInvite {
    name: String,
    expires_on: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum CreateResult {
    NameExists,
    InvalidExpiresOn,
    Created(InviteForm)
}

impl IntoResponse for CreateResult {
    fn into_response(self) -> Response {
        match self {
            Self::NameExists => (
                StatusCode::BAD_REQUEST,
                body::Json(self)
            ).into_response(),
            Self::InvalidExpiresOn => (
                StatusCode::BAD_REQUEST,
                body::Json(self),
            ).into_response(),
            Self::Created(invite) => (
                StatusCode::CREATED,
                body::Json(invite)
            ).into_response(),
        }
    }
}

pub async fn create_invite(
    state: state::SharedState,
    _initiator: Initiator,
    body::Json(NewInvite {
        name,
        expires_on,
    }): body::Json<NewInvite>,
) -> Result<CreateResult, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    // check if user has permission to create invites

    let now = Utc::now();

    if let Some(given) = expires_on.as_ref() {
        if *given < now {
            return Ok(CreateResult::InvalidExpiresOn);
        }
    }

    let token = InviteToken::gen();
    let issued_on = Utc::now();
    let status = InviteStatus::Pending;

    let result = transaction.execute(
        "\
        insert into user_invites (token, name, issued_on, expires_on, status) values \
        ($1, $2, $3, $4, $5)",
        &[&token, &name, &issued_on, &expires_on, &status]
    ).await;

    if let Err(err) = result {
        if let Some(kind) = db::ErrorKind::check(&err) {
            match kind {
                db::ErrorKind::Unique(constraint) => if constraint == "user_invites_name_key" {
                    return Ok(CreateResult::NameExists);
                },
                _ => {}
            }
        }

        return Err(error::Error::context_source(
            "failed to create invite",
            err
        ));
    }

    transaction.commit()
        .await
        .context("failed to create transaction")?;

    Ok(CreateResult::Created(InviteForm {
        token,
        name,
        issued_on,
        expires_on: InviteExpires {
            enabled: expires_on.is_some(),
            date: expires_on.unwrap_or_default(),
        },
        status
    }))
}

#[derive(Debug, Deserialize)]
pub struct UpdateInvite {
    name: String,
    expires_on: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum UpdateResult {
    NotFound,
    NotPending,
    NameExists,
    InvalidExpiresOn,
    Updated(InviteForm),
}

impl IntoResponse for UpdateResult {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                body::Json(self),
            ).into_response(),
            Self::NotPending => (
                StatusCode::BAD_REQUEST,
                body::Json(self),
            ).into_response(),
            Self::NameExists => (
                StatusCode::BAD_REQUEST,
                body::Json(self),
            ).into_response(),
            Self::InvalidExpiresOn => (
                StatusCode::BAD_REQUEST,
                body::Json(self),
            ).into_response(),
            Self::Updated(invite) => (
                StatusCode::OK,
                body::Json(invite)
            ).into_response()
        }
    }
}

pub async fn update_invite(
    state: state::SharedState,
    _initiator: Initiator,
    Path(InvitePath {
        token,
    }): Path<InvitePath>,
    body::Json(UpdateInvite {
        name,
        expires_on,
    }): body::Json<UpdateInvite>,
) -> Result<UpdateResult, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let result = Invite::retrieve(&transaction, &token)
        .await
        .context("failed to retrieve user invite")?;

    let now = Utc::now();

    let Some(Invite {
        issued_on,
        status,
        ..
    }) = result else {
        return Ok(UpdateResult::NotFound);
    };

    if !status.is_pending() {
        return Ok(UpdateResult::NotPending);
    }

    if let Some(given) = expires_on.as_ref() {
        if *given < now {
            return Ok(UpdateResult::InvalidExpiresOn);
        }
    }

    let result = transaction.execute(
        "\
        update user_invites \
        set name = $2, \
            expires_on = $3 \
        where token = $1",
        &[&token, &name, &expires_on]
    ).await;

    if let Err(err) = result {
        if let Some(kind) = db::ErrorKind::check(&err) {
            match kind {
                db::ErrorKind::Unique(constraint) => if constraint == "user_invites_name_key" {
                    return Ok(UpdateResult::NameExists);
                },
                _ => {}
            }
        }

        return Err(error::Error::context_source(
            "failed to update user invite",
            err
        ));
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(UpdateResult::Updated(InviteForm {
        token,
        name,
        issued_on,
        expires_on: InviteExpires {
            enabled: expires_on.is_some(),
            date: expires_on.unwrap_or_default(),
        },
        status
    }))
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum DeleteResult {
    NotFound,
    Deleted,
}

impl IntoResponse for DeleteResult {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                body::Json(self),
            ).into_response(),
            Self::Deleted => (
                StatusCode::OK,
                body::Json(self),
            ).into_response(),
        }
    }
}

pub async fn delete_invite(
    state: state::SharedState,
    _initiator: Initiator,
    Path(InvitePath {
        token,
    }): Path<InvitePath>,
) -> Result<DeleteResult, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let result = transaction.execute(
        "\
        delete from user_invites \
        where token = $1",
        &[&token]
    )
        .await
        .context("failed to delete user invite")?;

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    if result == 1 {
        Ok(DeleteResult::Deleted)
    } else {
        Ok(DeleteResult::NotFound)
    }
}
