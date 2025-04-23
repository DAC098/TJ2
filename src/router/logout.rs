use axum::http::{StatusCode, HeaderMap};
use axum::response::{IntoResponse, Response};

use crate::error::{self, Context};
use crate::sec::authn::{Session, Initiator, InitiatorError};
use crate::state;

pub async fn post(
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
        Err(err) => match err {
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
