use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use serde::Serialize;

use crate::db::ids::UserPeerId;
use crate::net::{Error, body};
use crate::net::header::{is_accepting_html, Location};
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
) -> Result<Response, Error> {
    if is_accepting_html(&headers)? {
        return Ok(Location::to("/settings/peer_client").into_response());
    }

    let conn = state.db_conn().await?;

    let peers = UserPeer::retrieve_many(&conn, &initiator.user.id).await?;

    futures::pin_mut!(peers);

    let mut rtn = Vec::new();

    while let Some(maybe) = peers.next().await {
        let record = maybe?;

        rtn.push(UserPeerPartial {
            id: record.id,
            name: record.name,
        });
    }

    Ok(body::Json(rtn).into_response())
}
