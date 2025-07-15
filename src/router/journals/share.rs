use std::collections::HashSet;

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{JournalId, JournalShareId, UserId};
use crate::journal::sharing;
use crate::journal::Journal;
use crate::net::body;
use crate::net::Error as NetError;
use crate::sec::authn::Initiator;
use crate::sec::authz::{self, Ability, Scope};
use crate::state;

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
    pub abilities: i64,
}

#[derive(Debug, strum::Display, Serialize, Deserialize)]
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

    authz::assert_permission(&conn, initiator.user.id, Scope::Journals, Ability::Update).await?;

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
               count(journal_share_abilities.journal_share_id) as abilities \
        from journal_shares \
            left join journal_share_users on \
                journal_shares.id = journal_share_users.journal_shares_id \
            left join journal_share_abilities on \
                journal_shares.id = journal_share_abilities.journal_shares_id \
        where journal_shares.journals_id = $2 \
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
            abilities: row.get(5),
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
    users: Vec<AttachedUser>,
    abilities: Vec<sharing::Ability>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachedUser {
    id: UserId,
    username: String,
    added: DateTime<Utc>,
}

#[derive(Debug, strum::Display, Serialize)]
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

    let journal = Journal::retrieve(&conn, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(NetError::Inner(RetrieveShareError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(NetError::from(authz::PermissionError::Denied));
    }

    let result = conn
        .query_opt(
            "\
        select journal_shares.id, \
               journal_shares.name, \
               journal_shares.created, \
               journal_shares.updated, \
        from journal_shares \
        where journal_shares.journals_id = $1 and \
              journal_shares.id = $2",
            &[&journal.id, &share_id],
        )
        .await?;

    let Some(row) = result else {
        return Err(NetError::Inner(RetrieveShareError::ShareNotFound));
    };

    let id = row.get(0);
    let name = row.get(1);
    let created = row.get(2);
    let updated = row.get(3);

    let users = {
        let params: db::ParamsArray<'_, 1> = [&id];

        let stream = conn
            .query_raw(
                "\
            select users.id, \
                   users.username, \
                   journal_share_users.added \
            from journal_share_users \
                left join users on \
                    journal_share_users.users_id = users.id \
            where journal_share_users.journal_shares_id = $1",
                params,
            )
            .await?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(maybe) = stream.next().await {
            let row = maybe?;

            rtn.push(AttachedUser {
                id: row.get(0),
                username: row.get(1),
                added: row.get(2),
            });
        }

        rtn
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
        users,
        abilities,
    }))
}

#[derive(Debug, Deserialize)]
pub struct NewJournalShare {
    name: String,
    abilities: Vec<sharing::Ability>,
    users: Vec<UserId>,
}

#[derive(Debug, strum::Display, Serialize)]
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
    body::Json(NewJournalShare {
        name,
        abilities,
        users,
    }): body::Json<NewJournalShare>,
) -> Result<body::Json<JournalShareFull>, NetError<CreateShareError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

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

    let attached_users = create_attached_users(&transaction, &id, &created, users).await?;
    let attached_abilities = create_abilities(&transaction, &id, abilities).await?;

    transaction.commit().await?;

    Ok(body::Json(JournalShareFull {
        id,
        name,
        created,
        updated: None,
        users: attached_users,
        abilities: attached_abilities,
    }))
}

async fn create_attached_users(
    conn: &impl db::GenericClient,
    journal_shares_id: &JournalShareId,
    created: &DateTime<Utc>,
    users: Vec<UserId>,
) -> Result<Vec<AttachedUser>, NetError<CreateShareError>> {
    let mut params: db::ParamsVec<'_> = vec![&journal_shares_id, &created];
    let mut query = String::from(
        "\
        with tmp_insert as ( \
            insert into journal_share_users (journal_shares_id, users_id) values \
    ",
    );

    for (index, id) in users.iter().enumerate() {
        if index > 0 {
            query.push_str(", ");
        }

        let statement = format!("($1, ${})", db::push_param(&mut params, id));

        query.push_str(&statement);
    }

    query.push_str(
        "\
        ) \
        select users.id, \
               users.username, \
               journal_share_users.added \
        from tmp_insert \
            left join users on \
                tmp_insert.users_id = users.id \
        order by users.username",
    );

    let stream = conn.query_raw(&query, params).await?;

    futures::pin_mut!(stream);

    let mut rtn = Vec::new();

    while let Some(maybe) = stream.next().await {
        let row = maybe?;

        rtn.push(AttachedUser {
            id: row.get(0),
            username: row.get(1),
            added: row.get(2),
        });
    }

    Ok(rtn)
}

async fn create_abilities(
    conn: &impl db::GenericClient,
    journal_shares_id: &JournalShareId,
    abilities: Vec<sharing::Ability>,
) -> Result<Vec<sharing::Ability>, NetError<CreateShareError>> {
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

    conn.execute(&query, &params).await?;

    Ok(Vec::from_iter(ability_set))
}

#[derive(Debug, strum::Display, Serialize)]
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

    let journal = Journal::retrieve(&transaction, (&journals_id, &initiator.user.id))
        .await?
        .ok_or(NetError::Inner(DeleteShareError::JournalNotFound))?;

    if journal.users_id != initiator.user.id {
        return Err(NetError::from(authz::PermissionError::Denied));
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

    let share = transaction
        .execute("delete from journal_shares where id = $1", &[&share_id])
        .await?;

    if share != 1 {
        return Err(NetError::Inner(DeleteShareError::ShareNotFound));
    }

    transaction.commit().await?;

    Ok(StatusCode::OK)
}
