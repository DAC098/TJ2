use axum::extract::Path;
use axum::http::{HeaderMap, Uri, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::{self, Context};
use crate::router::body;
use crate::router::macros;
use crate::state;
use crate::sec::authn::Initiator;
use crate::sync::RemoteServer;

#[derive(Debug, Serialize)]
pub struct PartialRemoteServer {
    pub id: db::ids::RemoteServerId,
    pub addr: String,
    pub port: u16,
    pub secure: bool,
}

pub async fn search_servers(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;
    let initiator = macros::require_initiator!(&transaction, &headers, Some(uri));

    macros::res_if_html!(state.templates(), &headers);

    // perform auth check to see if they have permission to view this page

    let params: db::ParamsArray<'_, 0> = [];
    let stream = transaction.query_raw(
        "\
        select remote_servers.id, \
               remote_servers.addr, \
               remote_servers.port, \
               remote_servers.secure \
        from remote_servers",
        params
    )
        .await
        .context("failed to query remote servers")?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(try_record) = stream.next().await {
        let record = try_record.context("failed to retrieve remote server record")?;

        rtn.push(PartialRemoteServer {
            id: record.get(0),
            addr: record.get(1),
            port: RemoteServer::get_port(record.get(2)),
            secure: record.get(3),
        });
    }

    transaction.rollback()
        .await
        .context("failed to rollback transaction")?;

    Ok((
        StatusCode::OK,
        body::Json(rtn)
    ).into_response())
}

#[derive(Debug, Deserialize)]
pub struct RemoteServerPath {
    server_id: db::ids::RemoteServerId,
}

#[derive(Debug, Serialize)]
pub struct RemoteServerForm {
    addr: String,
    port: u16,
    secure: bool,
}

pub async fn retrieve_server(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
    Path(RemoteServerPath {
        server_id
    }): Path<RemoteServerPath>,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let conn = state.db_conn().await?;
    let initiator = macros::require_initiator!(&conn, &headers, Some(uri));

    // perform auth check to see if they have permission to view this page

    let result = RemoteServer::retrieve(&conn, &server_id)
        .await
        .context("error when retrieving remote server")?;

    let Some(server) = result else {
        return Ok((StatusCode::NOT_FOUND).into_response());
    };

    Ok((
        StatusCode::OK,
        body::Json(RemoteServerForm {
            addr: server.addr,
            port: server.port,
            secure: server.secure,
        })
    ).into_response())
}

#[derive(Debug, Deserialize)]
pub struct NewRemoteServer {
    addr: String,
    port: u16,
    secure: bool
}

pub async fn create_server(
    state: state::SharedState,
    _initiator: Initiator,
    body::Json(NewRemoteServer {
        addr,
        port,
        secure,
    }): body::Json<NewRemoteServer>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let result = transaction.query_one(
        "\
        insert into remote_servers (addr, port, secure) values \
        ($1, $2, $3) \
        returning id",
        &[&addr, &db::U16toI32(&port)]
    )
        .await
        .context("failed to create remote server")?;

    let id: db::ids::RemoteServerId = result.get(0);

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok((
        StatusCode::CREATED,
        body::Json(RemoteServer {
            id,
            addr,
            port,
            secure,
        })
    ).into_response())
}

#[derive(Debug, Deserialize)]
pub struct UpdateRemoteServer {
    addr: String,
    port: u16,
    secure: bool,
}

pub async fn update_server(
    state: state::SharedState,
    _initiator: Initiator,
    Path(RemoteServerPath {
        server_id
    }): Path<RemoteServerPath>,
    body::Json(UpdateRemoteServer {
        addr,
        port,
        secure,
    }): body::Json<UpdateRemoteServer>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let Some(_) = RemoteServer::retrieve(&transaction, &server_id)
        .await
        .context("failed to retrieve remote server")? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let _ = transaction.execute(
        "\
        update remote_servers \
        set addr = $2, \
            port = $3, \
            secure = $4 \
        where id = $1",
        &[&server_id, &db::U16toI32(&port), &secure]
    )
        .await
        .context("failed to update remote server")?;

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok((
        StatusCode::OK,
        body::Json(RemoteServer {
            id: server_id,
            addr,
            port,
            secure,
        })
    ).into_response())
}

pub async fn delete_server(
    state: state::SharedState,
    _initiator: Initiator,
    Path(RemoteServerPath {
        server_id
    }): Path<RemoteServerPath>
) -> Result<StatusCode, error::Error> {
    Ok(StatusCode::OK)
}
