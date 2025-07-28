use std::collections::HashSet;

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use chrono::{DateTime, Utc};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{JournalId, JournalShareId};
use crate::journal::sharing::{self, JournalShare};
use crate::journal::Journal;
use crate::net::body;
use crate::net::Error as NetError;
use crate::router::handles;
use crate::sec::authn::Initiator;
use crate::sec::authz::{self, Ability, Scope};
use crate::state;

mod invite;
mod users;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/", get(search_shares).post(create_share))
        .route("/new", get(handles::send_html))
        .route(
            "/:share_id",
            get(retrieve_share).patch(update_share).delete(delete_share),
        )
        .route(
            "/:share_id/invite",
            get(invite::search_invites)
                .post(invite::create_invite)
                .delete(invite::delete_invite),
        )
        .route(
            "/:share_id/users",
            get(users::search_users).delete(users::remove_user),
        )
}

#[derive(Debug, Deserialize)]
pub struct JournalPath {
    journals_id: JournalId,
}

#[derive(Debug, Deserialize)]
pub struct SharePath {
    journals_id: JournalId,
    share_id: JournalShareId,
}

#[derive(Debug, Serialize)]
pub struct JournalSharePartial {
    pub id: JournalShareId,
    pub name: String,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
    pub users: i64,
    pub pending_invites: i64,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum ShareSearchError {
    JournalNotFound,
}

impl IntoResponse for ShareSearchError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn search_shares(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(JournalPath { journals_id }): Path<JournalPath>,
) -> Result<body::Json<Vec<JournalSharePartial>>, NetError<ShareSearchError>> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db().get().await?;

    authz::assert_permission(&conn, initiator.user.id, Scope::Journals, Ability::Read).await?;

    let journal = Journal::retrieve(&conn, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(NetError::Inner(ShareSearchError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(NetError::from(authz::PermissionError::Denied));
    }

    let params: db::ParamsArray<'_, 1> = [&journals_id];
    let stream = conn
        .query_raw(
            "\
        select journal_shares.id, \
               journal_shares.name, \
               journal_shares.created, \
               journal_shares.updated, \
               count(journal_share_users.users_id) as users, \
               count(journal_share_invites.status) as pending_invites \
        from journal_shares \
            left join journal_share_users on \
                journal_shares.id = journal_share_users.journal_shares_id \
            left join journal_share_invites on \
                journal_shares.id = journal_share_invites.journal_shares_id and
                journal_share_invites.status = 0 \
        where journal_shares.journals_id = $1 \
        group by journal_shares.id, \
                 journal_shares.name, \
                 journal_shares.created, \
                 journal_shares.updated \
        order by journal_shares.name",
            params,
        )
        .await?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(maybe) = stream.next().await {
        let row = maybe?;

        rtn.push(JournalSharePartial {
            id: row.get(0),
            name: row.get(1),
            created: row.get(2),
            updated: row.get(3),
            users: row.get(4),
            pending_invites: row.get(5),
        });
    }

    Ok(body::Json(rtn))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JournalShareFull {
    id: JournalShareId,
    name: String,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
    abilities: Vec<sharing::Ability>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum RetrieveShareError {
    JournalNotFound,
    ShareNotFound,
}

impl IntoResponse for RetrieveShareError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::ShareNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn retrieve_share(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(SharePath {
        journals_id,
        share_id,
    }): Path<SharePath>,
) -> Result<body::Json<JournalShareFull>, NetError<RetrieveShareError>> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db().get().await?;

    authz::assert_permission(&conn, initiator.user.id, Scope::Journals, Ability::Read).await?;

    let journal = Journal::retrieve(&conn, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(NetError::Inner(RetrieveShareError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(NetError::from(authz::PermissionError::Denied));
    }

    let Some(JournalShare {
        id,
        name,
        created,
        updated,
        ..
    }) = JournalShare::retrieve(&conn, (&journal.id, &share_id)).await?
    else {
        return Err(NetError::Inner(RetrieveShareError::ShareNotFound));
    };

    let abilities = {
        let params: db::ParamsArray<'_, 1> = [&id];

        let stream = conn
            .query_raw(
                "\
            select journal_share_abilities.ability \
            from journal_share_abilities \
            where journal_share_abilities.journal_shares_id = $1",
                params,
            )
            .await?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(maybe) = stream.next().await {
            let row = maybe?;

            rtn.push(row.get(0));
        }

        rtn
    };

    Ok(body::Json(JournalShareFull {
        id,
        name,
        created,
        updated,
        abilities,
    }))
}

#[derive(Debug, Deserialize)]
pub struct NewJournalShare {
    name: String,
    abilities: Vec<sharing::Ability>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum CreateShareError {
    JournalNotFound,
    NameAlreadyExists,
}

impl IntoResponse for CreateShareError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::NameAlreadyExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
        }
    }
}

pub async fn create_share(
    state: state::SharedState,
    initiator: Initiator,
    Path(JournalPath { journals_id }): Path<JournalPath>,
    body::Json(NewJournalShare { name, abilities }): body::Json<NewJournalShare>,
) -> Result<body::Json<JournalShareFull>, NetError<CreateShareError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        Scope::Journals,
        Ability::Update,
    )
    .await?;

    let journal = Journal::retrieve(&transaction, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(NetError::Inner(CreateShareError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(NetError::from(authz::PermissionError::Denied));
    }

    let created = Utc::now();
    let result = transaction
        .query_one(
            "\
        insert into journal_shares (journals_id, name, created) values \
        ($1, $2, $3) \
        returning id",
            &[&journal.id, &name, &created],
        )
        .await;

    let id: JournalShareId = match result {
        Ok(row) => row.get(0),
        Err(err) => {
            if let Some(kind) = db::ErrorKind::check(&err) {
                return match kind {
                    db::ErrorKind::Unique(constraint) => match constraint {
                        "journal_shares_journals_id_name_key" => {
                            Err(NetError::Inner(CreateShareError::NameAlreadyExists))
                        }
                        _ => Err(NetError::from(err)),
                    },
                    db::ErrorKind::ForeignKey(constraint) => match constraint {
                        "journal_shares_journals_id_fkey" => Err(NetError::message(
                            "journal id not found when journal was found",
                        )
                        .with_source(err)),
                        _ => Err(NetError::from(err)),
                    },
                };
            } else {
                return Err(NetError::from(err));
            }
        }
    };

    let abilities = create_abilities(&transaction, &id, abilities).await?;

    transaction.commit().await?;

    Ok(body::Json(JournalShareFull {
        id,
        name,
        created,
        updated: None,
        abilities,
    }))
}

async fn create_abilities(
    conn: &impl db::GenericClient,
    journal_shares_id: &JournalShareId,
    abilities: Vec<sharing::Ability>,
) -> Result<Vec<sharing::Ability>, NetError<CreateShareError>> {
    if abilities.is_empty() {
        return Ok(Vec::new());
    }

    let ability_set: HashSet<sharing::Ability> = HashSet::from_iter(abilities);
    let mut params: db::ParamsVec<'_> = vec![&journal_shares_id];
    let mut query = String::from(
        "\
        insert into journal_share_abilities (journal_shares_id, ability) values ",
    );

    for (index, id) in ability_set.iter().enumerate() {
        if index > 0 {
            query.push_str(", ");
        }

        let statement = format!("($1, ${})", db::push_param(&mut params, id));

        query.push_str(&statement);
    }

    tracing::debug!("create attached users query: {query}");

    conn.execute(&query, &params).await?;

    Ok(Vec::from_iter(ability_set))
}

#[derive(Debug, Deserialize)]
pub struct UpdateShare {
    name: String,
    abilities: Vec<sharing::Ability>,
}

#[derive(Debug, strum::Display, Serialize)]
pub enum UpdateShareError {
    JournalNotFound,
    ShareNotFound,
    NameAlreadyExists,
}

impl IntoResponse for UpdateShareError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::ShareNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::NameAlreadyExists => (StatusCode::BAD_REQUEST, body::Json(self)).into_response(),
        }
    }
}

pub async fn update_share(
    state: state::SharedState,
    initiator: Initiator,
    Path(SharePath {
        journals_id,
        share_id,
    }): Path<SharePath>,
    body::Json(UpdateShare { name, abilities }): body::Json<UpdateShare>,
) -> Result<body::Json<JournalShareFull>, NetError<UpdateShareError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        Scope::Journals,
        Ability::Update,
    )
    .await?;

    let journal = Journal::retrieve(&transaction, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(NetError::Inner(UpdateShareError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(NetError::from(authz::PermissionError::Denied));
    }

    let share = JournalShare::retrieve(&transaction, (&journal.id, &share_id))
        .await?
        .ok_or(NetError::Inner(UpdateShareError::ShareNotFound))?;

    let updated = Utc::now();
    let result = transaction
        .execute(
            "\
        update journal_shares \
        set name = $2, \
            updated = $3 \
        where id = $1",
            &[&share.id, &name, &updated],
        )
        .await;

    if let Err(err) = result {
        if let Some(kind) = db::ErrorKind::check(&err) {
            return match kind {
                db::ErrorKind::Unique(constraint) => match constraint {
                    "journal_shares_journals_id_name_key" => {
                        Err(NetError::Inner(UpdateShareError::NameAlreadyExists))
                    }
                    _ => Err(NetError::from(err)),
                },
                db::ErrorKind::ForeignKey(constraint) => match constraint {
                    "journal_shares_journals_id_fkey" => Err(NetError::message(
                        "journal id not found when journal was found",
                    )
                    .with_source(err)),
                    _ => Err(NetError::from(err)),
                },
            };
        } else {
            return Err(NetError::from(err));
        }
    }

    let abilities = update_abilities(&transaction, &share.id, abilities).await?;

    transaction.commit().await?;

    Ok(body::Json(JournalShareFull {
        id: share.id,
        name,
        created: share.created,
        updated: Some(updated),
        abilities,
    }))
}

async fn update_abilities(
    conn: &impl db::GenericClient,
    journal_shares_id: &JournalShareId,
    abilities: Vec<sharing::Ability>,
) -> Result<Vec<sharing::Ability>, NetError<UpdateShareError>> {
    if abilities.is_empty() {
        conn.execute(
            "delete from journal_share_abilities where journal_shares_id = $1",
            &[journal_shares_id],
        )
        .await?;

        return Ok(Vec::new());
    }

    let current = sharing::Ability::retrieve(conn, journal_shares_id)
        .await?
        .try_collect::<HashSet<sharing::Ability>>()
        .await?;

    let ability_set: HashSet<sharing::Ability> = HashSet::from_iter(abilities);

    let to_add = ability_set.difference(&current);
    let to_drop = current.difference(&ability_set);

    {
        let mut non_empty = false;
        let mut params: db::ParamsVec<'_> = vec![&journal_shares_id];
        let mut query = String::from(
            "\
            insert into journal_share_abilities (journal_shares_id, ability) values ",
        );

        for (index, id) in to_add.enumerate() {
            non_empty = true;

            if index > 0 {
                query.push_str(", ");
            }

            let statement = format!("($1, ${})", db::push_param(&mut params, id));

            query.push_str(&statement);
        }

        if non_empty {
            tracing::debug!("create attached users query: {query}");

            conn.execute(&query, &params).await?;
        }
    }

    {
        let list: Vec<&sharing::Ability> = to_drop.collect();

        if !list.is_empty() {
            conn.execute(
                "delete from journal_share_abilities where journal_shares_id = $1 and ability = any($2)",
                &[journal_shares_id, &list]
            ).await?;
        }
    }

    Ok(Vec::from_iter(ability_set))
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum DeleteShareError {
    JournalNotFound,
    ShareNotFound,
}

impl IntoResponse for DeleteShareError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::ShareNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

pub async fn delete_share(
    state: state::SharedState,
    initiator: Initiator,
    Path(SharePath {
        journals_id,
        share_id,
    }): Path<SharePath>,
) -> Result<StatusCode, NetError<DeleteShareError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        Scope::Journals,
        Ability::Delete,
    )
    .await?;

    let journal = Journal::retrieve(&transaction, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(NetError::Inner(DeleteShareError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(NetError::from(authz::PermissionError::Denied));
    }

    if JournalShare::retrieve(&transaction, (&journal.id, &share_id))
        .await?
        .is_none()
    {
        return Err(NetError::Inner(DeleteShareError::ShareNotFound));
    }

    let _abilities = transaction
        .execute(
            "delete from journal_share_abilities where journal_shares_id = $1",
            &[&share_id],
        )
        .await?;

    let _users = transaction
        .execute(
            "delete from journal_share_users where journal_shares_id = $1",
            &[&share_id],
        )
        .await?;

    let _invites = transaction
        .execute(
            "delete from journal_share_invites where journal_shares_id = $1",
            &[&share_id],
        )
        .await?;

    let share = transaction
        .execute("delete from journal_shares where id = $1", &[&share_id])
        .await?;

    if share != 1 {
        return Err(NetError::message(
            "failed to delete share when it should have been found",
        ));
    }

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
