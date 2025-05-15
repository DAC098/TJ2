pub async fn post(
    state: state::SharedState,
    initiator: ApiInitiator,
    body::Json(json): body::Json<()>
) -> Result<(), error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    tracing::debug!("received journal from client");
}
