use std::default::Default;

use axum::Router;
use axum::http::StatusCode;
use axum::response::{Response, IntoResponse};
use axum::routing::post;
use chrono::Utc;
use futures::StreamExt;
use serde::{Serialize, Deserialize};
use tj2_lib::sec::pki::{PublicKey, PrivateKey, PrivateKeyError};

use crate::db;
use crate::db::ids::{
    UserUid,
    JournalId,
    EntryId,
    CustomFieldUid,
    FileEntryUid,
    RemoteServerId,
    InviteToken,
};
use crate::error::{self, Context};
use crate::fs::RemovedFiles;
use crate::router::body;
use crate::journal::{self, Journal, FileStatus, FileEntry, CustomField};
use crate::sec;
use crate::sec::authn::ApiInitiator;
use crate::state::{self, Storage};
use crate::sync::{self, RemoteServer, PeerAddr};
use crate::sync::journal::{
    SyncEntryResult,
    EntryFileSync,
};
use crate::user::{self, UserBuilder, UserBuilderError};
use crate::user::invite::{Invite, InviteError};

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/register", post(register_peer_user))
        .route("/entries", post(receive_entry))
        .route("/journal", post(receive_journal))
}

async fn receive_journal(
    state: state::SharedState,
    initiator: ApiInitiator,
    body::Json(json): body::Json<sync::journal::JournalSync>
) -> Result<StatusCode, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let now = Utc::now();

    let journal = if let Some(mut exists) = Journal::retrieve(&transaction, &json.uid)
        .await
        .context("failed to retrieve journal")?
    {
        if exists.users_id != initiator.user.id {
            return Ok(StatusCode::BAD_REQUEST);
        }

        exists.updated = Some(now);
        exists.name = json.name;
        exists.description = json.description;

        exists.update(&transaction)
            .await
            .context("failed to update journal")?;

        exists
    } else {
        let mut options = Journal::create_options(initiator.user.id, json.name);
        options.uid(json.uid);

        if let Some(desc) = json.description {
            options.description(desc);
        }

        let journal = Journal::create(&transaction, options)
            .await
            .context("failed to create journal")?;

        journal
    };

    for cf in json.custom_fields {
        let mut options = CustomField::create_options(journal.id, cf.name, cf.config);
        options.uid = Some(cf.uid);
        options.order = cf.order;
        options.description = cf.description;

        CustomField::create(&transaction, options)
            .await
            .context("failed to create custom field")?;
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(StatusCode::CREATED)
}

async fn receive_entry(
    state: state::SharedState,
    initiator: ApiInitiator,
    body::Json(json): body::Json<sync::journal::EntrySync>,
) -> Result<SyncEntryResult, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    tracing::debug!("received entry from server: {} {json:#?}", json.uid);

    let journal = {
        let Some(result) = journal::Journal::retrieve(&transaction, &json.journals_uid)
            .await
            .context("failed to retrieve journal")? else {
            tracing::debug!("failed to retrieve journal: {}", json.journals_uid);

            return Ok(SyncEntryResult::JournalNotFound);
        };

        result
    };

    if journal.users_id != initiator.user.id {
        return Ok(SyncEntryResult::JournalNotFound);
    }

    let result = transaction.query_one(
        "\
        insert into entries (uid, journals_id, users_id, entry_date, title, contents, created, updated) \
        values ($1, $2, $3, $4, $5, $6, $7, $8) \
        on conflict (uid) do update \
            set entry_date = excluded.entry_date, \
                title = excluded.title, \
                contents = excluded.contents, \
                updated = excluded.updated \
        returning id",
        &[&json.uid, &journal.id, &initiator.user.id, &json.date, &json.title, &json.contents, &json.created, &json.updated]
    )
        .await
        .context("failed to upsert entry")?;

    let entries_id: EntryId = result.get(0);

    upsert_tags(&transaction, &entries_id, &json.tags).await?;

    {
        let UpsertCFS {
            not_found,
            invalid
        } = upsert_cfs(&transaction, &journal.id, &entries_id, &json.custom_fields).await?;

        if !not_found.is_empty() {
            return Ok(SyncEntryResult::CFNotFound {
                uids: not_found
            });
        }

        if !invalid.is_empty() {
            return Ok(SyncEntryResult::CFInvalid {
                uids: invalid
            });
        }
    }

    let mut removed_files = RemovedFiles::new();

    {
        let tmp_server_id = RemoteServerId::new(1).unwrap();
        let journal_dir = state.storage()
            .journal_dir(journal.id);

        let UpsertFiles {
            not_found
        } = upsert_files(
            &transaction,
            &entries_id,
            &tmp_server_id,
            journal_dir,
            json.files,
            &mut removed_files
        ).await?;

        if !not_found.is_empty() {
            return Ok(SyncEntryResult::FileNotFound {
                uids: not_found,
            });
        }
    }

    let result = transaction.commit()
        .await
        .context("failed to commit entry sync transaction");

    if let Err(err) = result {
        removed_files.log_rollback().await;

        return Err(err);
    }

    removed_files.log_clean().await;

    Ok(SyncEntryResult::Synced)
}

async fn upsert_tags(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
    tags: &Vec<sync::journal::EntryTagSync>
) -> Result<(), error::Error> {
    if !tags.is_empty() {
        let mut params: db::ParamsVec<'_> = vec![entries_id];
        let mut query = String::from(
            "with tmp_insert as ( \
                insert into entry_tags (entries_id, key, value, created, updated) \
                values "
        );

        for (index, tag) in tags.iter().enumerate() {
            if index != 0 {
                query.push_str(", ");
            }

            let statement = format!(
                "($1, ${}, ${}, ${}, ${})",
                db::push_param(&mut params, &tag.key),
                db::push_param(&mut params, &tag.value),
                db::push_param(&mut params, &tag.created),
                db::push_param(&mut params, &tag.updated),
            );

            query.push_str(&statement);
        }

        query.push_str(" on conflict (entries_id, key) do update \
                set key = excluded.key, \
                    value = excluded.value, \
                    updated = excluded.updated \
                returning entries_id, key \
            ) \
            delete from entry_tags \
            using tmp_insert \
            where entry_tags.entries_id = tmp_insert.entries_id and \
                  entry_tags.key != tmp_insert.key"
        );

        conn.execute(&query, params.as_slice())
            .await
            .context("failed to upsert tags")?;
    } else {
        conn.execute(
            "\
            delete from entry_tags \
            where entries_id = $1",
            &[entries_id]
        )
            .await
            .context("failed to delete tags")?;
    }

    Ok(())
}

struct UpsertCFS {
    not_found: Vec<CustomFieldUid>,
    invalid: Vec<CustomFieldUid>,
}

impl Default for UpsertCFS {
    fn default() -> Self {
        UpsertCFS {
            not_found: Vec::new(),
            invalid: Vec::new(),
        }
    }
}

async fn upsert_cfs(
    conn: &impl db::GenericClient,
    journals_id: &JournalId,
    entries_id: &EntryId,
    cfs: &Vec<sync::journal::EntryCFSync>,
) -> Result<UpsertCFS, error::Error> {
    let mut results = UpsertCFS::default();

    if !cfs.is_empty() {
        let fields = journal::CustomField::retrieve_journal_uid_map(conn, journals_id)
            .await
            .context("failed to retrieve journal custom fields")?;
        let mut counted = 0;
        let mut params: db::ParamsVec<'_> = vec![entries_id];
        let mut query = String::from(
            "\
            with tmp_insert as ( \
                insert into custom_field_entries (entries_id, custom_fields_id, value, created, updated) \
                values "
        );

        for (index, cf) in cfs.iter().enumerate() {
            let Some(field_ref) = fields.get(&cf.custom_fields_uid) else {
                results.not_found.push(cf.custom_fields_uid.clone());

                continue;
            };

            if index != 0 {
                query.push_str(", ");
            }

            let statement = format!(
                "($1, ${}, ${}, ${}, ${})",
                db::push_param(&mut params, &field_ref.id),
                db::push_param(&mut params, &cf.value),
                db::push_param(&mut params, &cf.created),
                db::push_param(&mut params, &cf.updated),
            );

            query.push_str(&statement);

            counted += 1;
        }

        if counted > 0 {
            query.push_str(" on conflict (custom_fields_id, entries_id) do update \
                    set value = excluded.value, \
                        updated = excluded.updated \
                    returning custom_fields_id, \
                              entries_id \
                ) \
                delete from custom_field_entries \
                using tmp_insert \
                where custom_field_entries.entries_id = tmp_insert.entries_id and \
                      custom_field_entries.custom_fields_id != tmp_insert.custom_fields_id"
            );

            //tracing::debug!("query: {query}");

            conn.execute(&query, params.as_slice())
                .await
                .context("failed to upsert custom fields")?;
        }
    } else {
        conn.execute(
            "\
            delete from custom_field_entries \
            where entries_id = $1",
            &[entries_id]
        )
            .await
            .context("failed to delete custom fields")?;
    }

    Ok(results)
}

#[derive(Debug, Default)]
struct UpsertFiles {
    not_found: Vec<FileEntryUid>
}

async fn upsert_files(
    conn: &impl db::GenericClient,
    entries_id: &EntryId,
    server_id: &RemoteServerId,
    journal_dir: journal::JournalDir,
    files: Vec<sync::journal::EntryFileSync>,
    removed_files: &mut RemovedFiles,
) -> Result<UpsertFiles, error::Error> {
    let status = FileStatus::Requested;
    let rtn = UpsertFiles::default();

    if !files.is_empty() {
        let mut known = FileEntry::retrieve_uid_map(conn, entries_id)
            .await
            .context("failed to retrieve known file entries")?;
        let mut counted = 0;
        let mut params: db::ParamsVec<'_> = vec![entries_id, &status, server_id];
        let mut query = String::from(
            "\
            insert into file_entries ( \
                entries_id, \
                status, \
                uid, \
                name, \
                mime_type, \
                mime_subtype, \
                mime_param, \
                size, \
                hash, \
                created, \
                updated \
            ) \
            values "
        );

        for (index, file) in files.iter().enumerate() {
            if let Some(exists) = known.remove(file.uid()) {
                // we know that the file exists for this entry so we will not 
                // need to check the entry id
                match exists {
                    FileEntry::Requested(_) => {
                        // the journal should not have these as the peer should
                        // only send received files and the local server should
                        // not be modifying the journal
                        return Err(error::Error::context(
                            "encountered requested file when removing local files"
                        ));
                    }
                    // skip the other entries as we do not need to worry about
                    // them
                    _ => {}
                }
            } else {
                // do a lookup to make sure that the uid exists for a different
                // entry
                let lookup_result = FileEntry::retrieve(conn, file.uid())
                    .await
                    .context("failed to lookup file uid")?;

                if let Some(found) = lookup_result {
                    match found {
                        FileEntry::Received(rec) => if rec.entries_id != *entries_id {
                            // the given uid exists but is not attached to the
                            // entry we are currently working on
                            continue;
                        },
                        FileEntry::Requested(_) => {
                            return Err(error::Error::context(
                                "encountered requested file when removing local files"
                            ));
                        }
                    }
                }
            }
            if index != 0 {
                query.push_str(", ");
            }

            match file {
                EntryFileSync::Received(rec) => {
                    let statement = format!(
                        "($1, $2, $3, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                        db::push_param(&mut params, &rec.uid),
                        db::push_param(&mut params, &rec.name),
                        db::push_param(&mut params, &rec.mime_type),
                        db::push_param(&mut params, &rec.mime_subtype),
                        db::push_param(&mut params, &rec.mime_param),
                        db::push_param(&mut params, &rec.size),
                        db::push_param(&mut params, &rec.hash),
                        db::push_param(&mut params, &rec.created),
                        db::push_param(&mut params, &rec.updated),
                    );

                    query.push_str(&statement);
                }
            }

            counted += 1;
        }

        if !known.is_empty() {
            // we will delete the entries that were not found in order to
            // prevent the posibility that a new record or an updated one has
            // a similar name
            let uids = known.into_keys()
                .collect::<Vec<FileEntryUid>>();

            tracing::debug!("deleting file entries: {}", uids.len());

            conn.execute(
                "delete from file_entries where uid = any($1)",
                &[&uids]
            )
                .await
                .context("failed to delete from file entries")?;
        }

        if counted > 0 {
            query.push_str(" on conflict (uid) do update \
                set name = excluded.name, \
                    updated = excluded.updated \
                returning id, \
                          entries_id"
            );

            tracing::debug!("upserting file entries: {counted} {query}");

            conn.execute(&query, params.as_slice())
                .await
                .context("failed to upsert files")?;
        }
    } else {
        // delete all files that are local to the manchine and just remove the
        // entries that are marked remote
        let known_files = FileEntry::retrieve_entry_stream(conn, entries_id)
            .await
            .context("failed to retrieve file entries")?;
        let mut ids = Vec::new();

        futures::pin_mut!(known_files);

        while let Some(try_record) = known_files.next().await {
            let file = try_record.context("failed to retrieve record")?;

            match file {
                FileEntry::Received(rec) => {
                    removed_files.add(journal_dir.file_path(&rec.id))
                        .await
                        .context("failed to remove received journal file")?;

                    ids.push(rec.id);
                }
                FileEntry::Requested(_) => {
                    return Err(error::Error::context(
                        "encountered requested file when removing local files"
                    ));
                }
            }
        }

        conn.execute(
            "delete from file_entries where entries_id = $1",
            &[entries_id]
        )
            .await
            .context("failed to delete file entries")?;
    }

    Ok(rtn)
}

#[derive(Debug, Deserialize)]
pub struct RegisterPeerUser {
    token: InviteToken,
    user: RegisterUser,
    peer: RegisterPeer,
}

#[derive(Debug, Deserialize)]
pub struct RegisterUser {
    uid: UserUid,
    username: String,
    password: String,
    confirm: String,
    public_key: PublicKey,
}

#[derive(Debug, Deserialize)]
pub struct RegisterPeer {
    addr: PeerAddr,
    port: u16,
    public_key: PublicKey,
}

#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "type")]
pub enum RegisterPeerUserError {
    #[error("the requested invite was not found")]
    InviteNotFound,

    #[error("the requested invite has already been used")]
    InviteUsed,

    #[error("the requested invite has expired")]
    InviteExpired,

    #[error("the confirm does not equal password")]
    InvalidConfirm,

    #[error("the specified username already exists")]
    UsernameExists,

    #[error("the specified user uid already exists")]
    UserUidExists,

    #[error("server address already exists")]
    ServerAddrExists,

    #[error("invalid server address")]
    InvalidServerAddr,

    #[serde(skip)]
    #[error(transparent)]
    Db(#[from] db::PgError),

    #[serde(skip)]
    #[error(transparent)]
    DbPool(#[from] db::PoolError),

    #[serde(skip)]
    #[error(transparent)]
    Argon(#[from] sec::password::HashError),

    #[serde(skip)]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[serde(skip)]
    #[error(transparent)]
    Pki(#[from] PrivateKeyError),

    #[serde(skip)]
    #[error(transparent)]
    Error(#[from] error::Error),
}

impl IntoResponse for RegisterPeerUserError {
    fn into_response(self) -> Response {
        error::log_error(&self);

        let status = match &self {
            Self::InviteNotFound => StatusCode::NOT_FOUND,
            Self::UsernameExists |
            Self::InvalidConfirm |
            Self::InviteExpired |
            Self::InviteUsed |
            Self::UserUidExists |
            Self::ServerAddrExists |
            Self::InvalidServerAddr => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, body::Json(self)).into_response()
    }
}

impl From<UserBuilderError> for RegisterPeerUserError {
    fn from(err: UserBuilderError) -> Self {
        match err {
            UserBuilderError::Argon(argon_err) =>
                RegisterPeerUserError::Argon(argon_err),
            UserBuilderError::UsernameExists =>
                RegisterPeerUserError::UsernameExists,
            UserBuilderError::UidExists =>
                RegisterPeerUserError::UserUidExists,
            UserBuilderError::Db(db_err) =>
                RegisterPeerUserError::Db(db_err)
        }
    }
}

pub async fn register_peer_user(
    state: state::SharedState,
    body::Json(RegisterPeerUser {
        token,
        user,
        peer,
    }): body::Json<RegisterPeerUser>,
) -> Result<(), RegisterPeerUserError> {
    let mut conn = state.db().get().await?;
    let transaction = conn.transaction().await?;

    let mut invite = Invite::retrieve(&transaction, &token)
        .await?
        .ok_or(RegisterPeerUserError::InviteNotFound)?;

    if !invite.status.is_pending() {
        return Err(RegisterPeerUserError::InviteUsed);
    }

    if invite.is_expired() {
        return Err(RegisterPeerUserError::InviteExpired);
    }

    let peer = register_peer(&transaction, peer).await?;
    let user = register_user(&transaction, state.storage(), &peer, user).await?;

    invite.mark_accepted(&transaction, &user.id)
        .await
        .map_err(|err| match err {
            InviteError::Db(db) => RegisterPeerUserError::Db(db),
            _ => unreachable!("invite pre-check failed {err}")
        })?;

    Ok(())
}

pub async fn register_user(
    conn: &impl db::GenericClient,
    storage: &Storage,
    server: &RemoteServer,
    RegisterUser {
        uid,
        username,
        password,
        confirm,
        public_key
    }: RegisterUser
) -> Result<user::User, RegisterPeerUserError> {
    if password != confirm {
        return Err(RegisterPeerUserError::InvalidConfirm);
    }

    let mut builder = UserBuilder::new_password(username, password)?;
    builder.with_uid(uid);

    let user = builder.build(conn).await?;

    conn.execute(
        "\
        insert into remote_server_users (server_id, users_id, public_key) values \
        ($1, $2, $3)",
        &[&server.id, &user.id, &db::ToBytea(&public_key)]
    ).await?;

    let user_dir = storage.user_dir(user.id);
    user_dir.create().await?;

    let private_key = PrivateKey::generate()?;
    private_key.save(user_dir.private_key(), false).await?;

    Ok(user)
}

pub async fn register_peer(
    conn: &impl db::GenericClient,
    RegisterPeer {
        addr,
        port,
        public_key,
    }: RegisterPeer
) -> Result<RemoteServer, RegisterPeerUserError> {
    if let Some(exists) = RemoteServer::retrieve(conn, &public_key).await? {
        return Ok(exists);
    }

    let Some(addr) = addr.to_valid_string() else {
        return Err(RegisterPeerUserError::InvalidServerAddr);
    };

    let result = conn.query_one(
        "\
        insert into remote_servers (addr, port, secure, public_key) values \
        ($1, $2, $3, $4) \
        returning id",
        &[&addr, &db::U16toI32(&port), &db::ToBytea(&public_key)]
    ).await;

    let record = result.map_err(|err| if let Some(kind) = db::ErrorKind::check(&err) {
        match kind {
            db::ErrorKind::Unique(constraint) => match constraint {
                "remote_servers_addr_key" => RegisterPeerUserError::ServerAddrExists,
                _ => err.into()
            }
            _ => err.into()
        }
    } else {
        err.into()
    })?;

    Ok(RemoteServer {
        id: record.get(0),
        addr,
        port,
        secure: false,
        public_key,
    })
}
