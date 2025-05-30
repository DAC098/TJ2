use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db::ids::UserPeerId;
use crate::error::{self, Context};
use crate::header::{is_accepting_html, Location};
use crate::router::{body, macros};
use crate::sec::authn::Initiator;
use crate::state;
use crate::user::peer::UserPeer;

#[derive(Debug, Serialize)]
pub struct UserPeerPartial {
    id: UserPeerId,
    name: String,
}

pub async fn get(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    if is_accepting_html(&headers).unwrap_or(true) {
        return Ok(Location::to("/settings/peer_client").into_response());
    }

    let conn = state.db_conn().await?;

    let peers = UserPeer::retrieve_many(&conn, &initiator.user.id)
        .await
        .context("failed to retrieve user peers")?;

    futures::pin_mut!(peers);

    let mut rtn = Vec::new();

    while let Some(maybe) = peers.next().await {
        let record = maybe.context("failed to retrieve user peer record")?;

        rtn.push(UserPeerPartial {
            id: record.id,
            name: record.name,
        });
    }

    Ok(body::Json(rtn).into_response())
}
