use std::collections::{HashMap, HashSet};

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::ids::{CustomFieldId, CustomFieldUid, JournalId, JournalUid, UserId, UserPeerId};
use crate::error;
use crate::jobs;
use crate::journal::{
    assert_permission, custom_field, CustomField, CustomFieldBuilder, Journal, JournalCreateError,
    JournalUpdateError,
};
use crate::net::body;
use crate::net::Error as NetError;
use crate::router::handles;
use crate::sec::authn::Initiator;
use crate::sec::authz::{self, Ability, Scope};
use crate::state;
use crate::user::peer::UserPeer;

mod entries;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/", get(retrieve_journals).post(create_journal))
        .route("/new", get(handles::send_html))
        .route("/:journals_id", get(retrieve_journal).patch(update_journal))
        .route(
            "/:journals_id/entries",
            get(entries::retrieve_entries).post(entries::create_entry),
        )
        .route("/:journals_id/entries/new", get(entries::retrieve_entry))
        .route(
            "/:journals_id/entries/:entries_id",
            get(entries::retrieve_entry)
                .patch(entries::update_entry)
                .delete(entries::delete_entry),
        )
        .route(
            "/:journals_id/entries/:entries_id/:file_entry_id",
            get(entries::files::retrieve_file).put(entries::files::upload_file),
        )
        .route("/:journals_id/sync", post(sync_with_remote))
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub struct JournalPartial {
    id: JournalId,
    uid: JournalUid,
    users_id: UserId,
    name: String,
    description: Option<String>,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
    has_peers: bool,
}

async fn retrieve_journals(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
) -> Result<Response, NetError> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db().get().await?;

    authz::assert_permission(&conn, initiator.user.id, Scope::Journals, Ability::Read).await?;

    let params: db::ParamsArray<'_, 1> = [&initiator.user.id];
    let journals = conn
        .query_raw(
            "\
        select journals.id, \
               journals.uid, \
               journals.users_id, \
               journals.name, \
               journals.description, \
               journals.created, \
               journals.updated, \
               count(journal_peers.user_peers_id) > 0 as has_peers \
        from journals \
            left join journal_peers on \
                journals.id = journal_peers.journals_id \
        where journals.users_id = $1 \
        group by journals.id, \
                 journals.uid, \
                 journals.users_id, \
                 journals.name, \
                 journals.description, \
                 journals.created, \
                 journals.updated \
        order by journals.name",
            params,
        )
        .await?;

    futures::pin_mut!(journals);

    let mut found = Vec::new();

    while let Some(try_record) = journals.next().await {
        let record = try_record?;

        found.push(JournalPartial {
            id: record.get(0),
            uid: record.get(1),
            users_id: record.get(2),
            name: record.get(3),
            description: record.get(4),
            created: record.get(5),
            updated: record.get(6),
            has_peers: record.get(7),
        });
    }

    Ok(body::Json(found).into_response())
}

#[derive(Debug, Deserialize)]
pub struct JournalPath {
    journals_id: JournalId,
}

#[derive(Debug, Serialize)]
pub struct CustomFieldFull {
    pub id: CustomFieldId,
    pub uid: CustomFieldUid,
    pub name: String,
    pub order: i32,
    pub config: custom_field::Type,
    pub description: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

impl From<CustomField> for CustomFieldFull {
    fn from(
        CustomField {
            id,
            uid,
            name,
            order,
            config,
            description,
            created,
            updated,
            ..
        }: CustomField,
    ) -> Self {
        Self {
            id,
            uid,
            name,
            order,
            config,
            description,
            created,
            updated,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct JournalPeer {
    user_peers_id: UserPeerId,
    name: String,
    synced: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct JournalFull {
    id: JournalId,
    uid: JournalUid,
    users_id: UserId,
    name: String,
    description: Option<String>,
    custom_fields: Vec<CustomFieldFull>,
    peers: Vec<JournalPeer>,
    created: DateTime<Utc>,
    updated: Option<DateTime<Utc>>,
}

impl From<(Journal, Vec<CustomFieldFull>, Vec<JournalPeer>)> for JournalFull {
    fn from(
        (local, custom_fields, peers): (Journal, Vec<CustomFieldFull>, Vec<JournalPeer>),
    ) -> Self {
        Self {
            id: local.id,
            uid: local.uid,
            users_id: local.users_id,
            name: local.name,
            description: local.description,
            custom_fields,
            peers,
            created: local.created,
            updated: local.updated,
        }
    }
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum RetrieveJournalError {
    JournalNotFound,
}

impl IntoResponse for RetrieveJournalError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

async fn retrieve_journal(
    state: state::SharedState,
    initiator: Initiator,
    headers: HeaderMap,
    Path(JournalPath { journals_id }): Path<JournalPath>,
) -> Result<body::Json<JournalFull>, NetError<RetrieveJournalError>> {
    body::assert_html(state.templates(), &headers)?;

    let conn = state.db().get().await?;

    let journal = Journal::retrieve_id(&conn, &journals_id, &initiator.user.id)
        .await?
        .ok_or(NetError::Inner(RetrieveJournalError::JournalNotFound))?;

    assert_permission(&conn, &initiator, &journal, Scope::Journals, Ability::Read).await?;

    let custom_fields = {
        let fields = CustomField::retrieve_journal_stream(&conn, &journals_id).await?;

        futures::pin_mut!(fields);

        let mut rtn = Vec::new();

        while let Some(try_record) = fields.next().await {
            let record = try_record?;

            rtn.push(CustomFieldFull {
                id: record.id,
                uid: record.uid,
                name: record.name,
                order: record.order,
                config: record.config,
                description: record.description,
                created: record.created,
                updated: record.updated,
            });
        }

        rtn
    };

    let peers = {
        let peers = UserPeer::retrieve_many(&conn, &journals_id).await?;

        futures::pin_mut!(peers);

        let mut rtn = Vec::new();

        while let Some(maybe) = peers.next().await {
            let record = maybe?;

            rtn.push(JournalPeer {
                user_peers_id: record.id,
                name: record.name,
                synced: None,
            });
        }

        rtn
    };

    Ok(body::Json(JournalFull::from((
        journal,
        custom_fields,
        peers,
    ))))
}

#[derive(Debug, Deserialize)]
pub struct NewCustomField {
    name: String,
    order: i32,
    config: custom_field::Type,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewJournal {
    name: String,
    description: Option<String>,
    custom_fields: Vec<NewCustomField>,
    peers: Vec<UserPeerId>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum CreateJournalError {
    NameExists,
    DuplicateCustomFields { duplicates: Vec<String> },
    PeersNotFound { ids: Vec<UserPeerId> },
}

impl IntoResponse for CreateJournalError {
    fn into_response(self) -> Response {
        match self {
            Self::NameExists | Self::DuplicateCustomFields { .. } | Self::PeersNotFound { .. } => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
        }
    }
}

async fn create_journal(
    state: state::SharedState,
    initiator: Initiator,
    body::Json(json): body::Json<NewJournal>,
) -> Result<(StatusCode, body::Json<JournalFull>), NetError<CreateJournalError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        Scope::Journals,
        Ability::Create,
    )
    .await?;

    let mut options = Journal::create_options(initiator.user.id, json.name);

    if let Some(description) = json.description {
        options.description(description);
    }

    let result = Journal::create(&transaction, options).await;

    let journal = match result {
        Ok(journal) => journal,
        Err(err) => match err {
            JournalCreateError::NameExists => {
                return Err(NetError::Inner(CreateJournalError::NameExists))
            }
            JournalCreateError::UidExists => return Err(NetError::general().with_source(err)),
            JournalCreateError::UserNotFound => return Err(NetError::general().with_source(err)),
            JournalCreateError::Db(err) => return Err(err.into()),
        },
    };

    let custom_fields = create_custom_fields(&transaction, &journal, json.custom_fields).await?;
    let peers =
        create_journal_peers(&transaction, &initiator.user.id, &journal.id, json.peers).await?;

    let journal_dir = state.storage().journal_dir(journal.id);

    let root_dir = journal_dir.create_root_dir().await?;

    let files_dir = match journal_dir.create_files_dir().await {
        Ok(files) => files,
        Err(err) => {
            if let Err(root_err) = tokio::fs::remove_dir(&root_dir).await {
                error::log_prefix_error("failed to remove journal root dir", &root_err);
            }

            return Err(err.into());
        }
    };

    if let Err(err) = transaction.commit().await {
        if let Err(files_err) = tokio::fs::remove_dir(&files_dir).await {
            error::log_prefix_error("failed to remove journal files dir", &files_err);
        } else if let Err(root_err) = tokio::fs::remove_dir(&root_dir).await {
            error::log_prefix_error("failed to remove journal root dir", &root_err);
        }

        return Err(err.into());
    }

    Ok((
        StatusCode::CREATED,
        body::Json(JournalFull::from((journal, custom_fields, peers))),
    ))
}

#[derive(Debug, Deserialize)]
pub struct ExistingCustomField {
    id: CustomFieldId,
    name: String,
    order: i32,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UpdateCustomField {
    Existing(ExistingCustomField),
    New(NewCustomField),
}

#[derive(Debug, Deserialize)]
pub struct UpdateJournal {
    name: String,
    description: Option<String>,
    custom_fields: Vec<UpdateCustomField>,
    peers: Vec<UserPeerId>,
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum UpdateJournalError {
    JournalNotFound,
    NameExists,
    CustomFieldNotFound { ids: Vec<CustomFieldId> },
    DuplicateCustomFields { duplicates: Vec<String> },
    PeersNotFound { ids: Vec<UserPeerId> },
}

impl IntoResponse for UpdateJournalError {
    fn into_response(self) -> Response {
        match self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
            Self::NameExists
            | Self::CustomFieldNotFound { .. }
            | Self::DuplicateCustomFields { .. }
            | Self::PeersNotFound { .. } => {
                (StatusCode::BAD_REQUEST, body::Json(self)).into_response()
            }
        }
    }
}

async fn update_journal(
    state: state::SharedState,
    initiator: Initiator,
    Path(JournalPath { journals_id }): Path<JournalPath>,
    body::Json(json): body::Json<UpdateJournal>,
) -> Result<body::Json<JournalFull>, NetError<UpdateJournalError>> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    authz::assert_permission(
        &transaction,
        initiator.user.id,
        Scope::Journals,
        Ability::Update,
    )
    .await?;

    let mut journal = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
        .await?
        .ok_or(NetError::Inner(UpdateJournalError::JournalNotFound))?;

    journal.name = json.name;
    journal.description = json.description;
    journal.updated = Some(Utc::now());

    if let Err(err) = journal.update(&transaction).await {
        return Err(match err {
            JournalUpdateError::NameExists => NetError::Inner(UpdateJournalError::NameExists),
            JournalUpdateError::NotFound => NetError::Inner(UpdateJournalError::JournalNotFound),
            JournalUpdateError::Db(err) => err.into(),
        });
    }

    let custom_fields = update_custom_fields(&transaction, &journal, json.custom_fields).await?;
    let peers =
        upsert_journal_peers(&transaction, &initiator.user.id, &journal.id, json.peers).await?;

    transaction.commit().await?;

    Ok(body::Json(JournalFull::from((
        journal,
        custom_fields,
        peers,
    ))))
}

async fn create_custom_fields(
    conn: &impl db::GenericClient,
    journal: &Journal,
    new_fields: Vec<NewCustomField>,
) -> Result<Vec<CustomFieldFull>, NetError<CreateJournalError>> {
    if new_fields.is_empty() {
        return Ok(Vec::new());
    }

    let created = Utc::now();

    let mut builders = Vec::new();
    let mut duplicates = Vec::new();
    let mut existing_names = HashSet::new();

    for field in new_fields {
        if !existing_names.insert(field.name.clone()) {
            duplicates.push(field.name);

            continue;
        }

        if !duplicates.is_empty() {
            continue;
        }

        let mut builder = CustomField::builder(journal.id, field.name, field.config);
        builder.with_uid(CustomFieldUid::gen());
        builder.with_order(field.order);
        builder.with_created(created);

        if let Some(desc) = field.description {
            builder.with_description(desc);
        }

        builders.push(builder);
    }

    if !duplicates.is_empty() {
        return Err(NetError::Inner(CreateJournalError::DuplicateCustomFields {
            duplicates,
        }));
    }

    if let Some(stream) = CustomFieldBuilder::build_many(conn, builders).await? {
        futures::pin_mut!(stream);

        let mut rtn = Vec::new();

        while let Some(record) = stream.next().await {
            rtn.push(CustomFieldFull::from(record?));
        }

        Ok(rtn)
    } else {
        Ok(Vec::new())
    }
}

async fn update_custom_fields(
    conn: &impl db::GenericClient,
    journal: &Journal,
    update_fields: Vec<UpdateCustomField>,
) -> Result<Vec<CustomFieldFull>, NetError<UpdateJournalError>> {
    let mut existing: HashMap<CustomFieldId, CustomField> = HashMap::new();
    let stream = CustomField::retrieve_journal_stream(conn, &journal.id).await?;

    futures::pin_mut!(stream);

    while let Some(maybe) = stream.next().await {
        let record = maybe?;

        existing.insert(record.id, record);
    }

    let created = Utc::now();
    let mut rtn = Vec::new();
    let mut not_found = Vec::new();
    let mut duplicates = Vec::new();
    let mut update_records = Vec::new();
    let mut insert_records = Vec::new();
    let mut existing_names = HashSet::new();

    for field in update_fields {
        match field {
            UpdateCustomField::Existing(existing_field) => {
                let Some(mut found) = existing.remove(&existing_field.id) else {
                    not_found.push(existing_field.id);

                    continue;
                };

                if !existing_names.insert(existing_field.name.clone()) {
                    duplicates.push(existing_field.name);

                    continue;
                }

                if !not_found.is_empty() {
                    continue;
                }

                if !duplicates.is_empty() {
                    continue;
                }

                found.name = existing_field.name;
                found.order = existing_field.order;
                found.description = existing_field.description;
                found.updated = Some(created);

                update_records.push(found);
            }
            UpdateCustomField::New(new_field) => {
                if !existing_names.insert(new_field.name.clone()) {
                    duplicates.push(new_field.name);

                    continue;
                }

                if !not_found.is_empty() {
                    continue;
                }

                if !duplicates.is_empty() {
                    continue;
                }

                let mut builder =
                    CustomField::builder(journal.id, new_field.name, new_field.config);
                builder.with_order(new_field.order);
                builder.with_uid(CustomFieldUid::gen());
                builder.with_created(created);

                if let Some(desc) = new_field.description {
                    builder.with_description(desc);
                }

                insert_records.push(builder);
            }
        }
    }

    if !duplicates.is_empty() {
        return Err(NetError::Inner(UpdateJournalError::DuplicateCustomFields {
            duplicates,
        }));
    }

    if !not_found.is_empty() {
        return Err(NetError::Inner(UpdateJournalError::CustomFieldNotFound {
            ids: not_found,
        }));
    }

    if !existing.is_empty() {
        let ids: Vec<CustomFieldId> = existing.into_keys().collect();

        conn.execute("delete from custom_fields where id = any($1)", &[&ids])
            .await?;
    }

    if let Some(stream) = CustomFieldBuilder::build_many(conn, insert_records).await? {
        futures::pin_mut!(stream);

        while let Some(maybe) = stream.next().await {
            rtn.push(CustomFieldFull::from(maybe?));
        }
    }

    if !update_records.is_empty() {
        {
            let mut await_list = futures::stream::FuturesUnordered::new();

            for existing in &update_records {
                await_list.push(existing.update(conn));
            }

            let mut failed = false;

            while let Some(result) = await_list.next().await {
                if let Err(err) = result {
                    error::log_prefix_error("failed to update custom_field", &err);

                    failed = true;
                }
            }

            if failed {
                return Err(NetError::general());
            }
        }

        rtn.extend(update_records.into_iter().map(CustomFieldFull::from));
    }

    Ok(rtn)
}

async fn create_journal_peers(
    conn: &impl db::GenericClient,
    users_id: &UserId,
    journals_id: &JournalId,
    list: Vec<UserPeerId>,
) -> Result<Vec<JournalPeer>, NetError<CreateJournalError>> {
    if list.is_empty() {
        return Ok(Vec::new());
    }

    let peers: HashMap<UserPeerId, UserPeer> = {
        let mut rtn = HashMap::new();
        let stream = UserPeer::retrieve_many(conn, users_id).await?;

        futures::pin_mut!(stream);

        while let Some(maybe) = stream.next().await {
            let record = maybe?;

            rtn.insert(record.id, record);
        }

        rtn
    };

    let mut rtn = Vec::with_capacity(list.len());
    let mut collected: HashSet<UserPeerId> = HashSet::new();
    let mut not_found = Vec::new();
    let mut params: db::ParamsVec<'_> = vec![journals_id];
    let mut query = String::from("insert into journal_peers (journals_id, user_peers_id) values ");

    for (index, id) in list.iter().enumerate() {
        if let Some(peer) = peers.get(id) {
            if !collected.insert(*id) {
                continue;
            }

            rtn.push(JournalPeer {
                user_peers_id: *id,
                name: peer.name.clone(),
                synced: None,
            });
        } else {
            not_found.push(*id);

            continue;
        }

        if index != 0 {
            query.push_str(", ");
        }

        let s = format!("($1, ${})", db::push_param(&mut params, id));

        query.push_str(&s);
    }

    if !not_found.is_empty() {
        return Err(NetError::Inner(CreateJournalError::PeersNotFound {
            ids: not_found,
        }));
    }

    conn.execute(&query, params.as_slice()).await?;

    Ok(rtn)
}

async fn upsert_journal_peers(
    conn: &impl db::GenericClient,
    users_id: &UserId,
    journals_id: &JournalId,
    list: Vec<UserPeerId>,
) -> Result<Vec<JournalPeer>, NetError<UpdateJournalError>> {
    let mut rtn = Vec::with_capacity(list.len());

    if list.is_empty() {
        return Ok(rtn);
    }

    let peers: HashMap<UserPeerId, UserPeer> = {
        let stream = UserPeer::retrieve_many(conn, users_id).await?;

        futures::pin_mut!(stream);

        let mut rtn = HashMap::new();

        while let Some(maybe) = stream.next().await {
            let record = maybe?;

            rtn.insert(record.id, record);
        }

        rtn
    };

    let mut collected: HashSet<UserPeerId> = HashSet::new();
    let mut not_found = Vec::new();
    let mut params: db::ParamsVec<'_> = vec![journals_id];
    let mut query = String::from(
        "\
        with tmp_insert as ( \
            insert into journal_peers (journals_id, user_peers_id) values ",
    );

    for (index, id) in list.iter().enumerate() {
        if let Some(peer) = peers.get(id) {
            if !collected.insert(*id) {
                continue;
            }

            rtn.push(JournalPeer {
                user_peers_id: *id,
                name: peer.name.clone(),
                synced: None,
            });
        } else {
            not_found.push(*id);

            continue;
        }

        if index != 0 {
            query.push_str(", ");
        }

        let s = format!("($1, ${})", db::push_param(&mut params, id));

        query.push_str(&s);
    }

    if !not_found.is_empty() {
        return Err(NetError::Inner(UpdateJournalError::PeersNotFound {
            ids: not_found,
        }));
    }

    query.push_str(
        " \
            on conflict (journals_id, user_peers_id) \
                do nothing \
            returning user_peers_id \
        ) \
        delete from journal_peers \
        using tmp_insert \
        where journal_peers.journals_id = $1 and \
              journal_peers.user_peers_id != tmp_insert.user_peers_id",
    );

    conn.execute(&query, params.as_slice()).await?;

    Ok(rtn)
}

#[derive(Debug, Deserialize)]
pub struct SyncOptions {}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum SyncResult {
    Queued { successful: Vec<String> },
    Noop,
}

impl IntoResponse for SyncResult {
    fn into_response(self) -> Response {
        match &self {
            Self::Noop | Self::Queued { .. } => (StatusCode::OK, body::Json(self)).into_response(),
        }
    }
}

#[derive(Debug, strum::Display, Serialize)]
#[serde(tag = "error")]
pub enum SyncError {
    JournalNotFound,
}

impl IntoResponse for SyncError {
    fn into_response(self) -> Response {
        match &self {
            Self::JournalNotFound => (StatusCode::NOT_FOUND, body::Json(self)).into_response(),
        }
    }
}

async fn sync_with_remote(
    state: state::SharedState,
    initiator: Initiator,
    Path(JournalPath { journals_id }): Path<JournalPath>,
    body::Json(_): body::Json<SyncOptions>,
) -> Result<SyncResult, NetError<SyncError>> {
    let conn = state.db_conn().await?;

    authz::assert_permission(&conn, initiator.user.id, Scope::Journals, Ability::Update).await?;

    let journal = Journal::retrieve_id(&conn, &journals_id, &initiator.user.id)
        .await?
        .ok_or(NetError::Inner(SyncError::JournalNotFound))?;

    let peers = UserPeer::retrieve_many(&conn, &journal.id).await?;

    futures::pin_mut!(peers);

    let mut successful = Vec::new();

    while let Some(maybe) = peers.next().await {
        let peer = maybe?;

        successful.push(peer.name.clone());

        tokio::spawn(jobs::sync::kickoff_send_journal(
            state.clone(),
            peer,
            journal.clone(),
        ));
    }

    if successful.is_empty() {
        Ok(SyncResult::Noop)
    } else {
        Ok(SyncResult::Queued { successful })
    }
}
