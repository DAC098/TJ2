use std::collections::{HashSet, HashMap};

use axum::Router;
use axum::extract::Path;
use axum::http::{StatusCode, Uri, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use chrono::{Utc, DateTime};
use futures::StreamExt;
use serde::{Serialize, Deserialize};

use crate::state;
use crate::db;
use crate::db::ids::{
    JournalId,
    JournalUid,
    UserId,
    CustomFieldId,
    CustomFieldUid,
};
use crate::error::{self, Context};
use crate::journal::{
    custom_field,
    Journal,
    JournalCreateError,
    JournalUpdateError,
    CustomField,
};
use crate::router::body;
use crate::router::macros;
use crate::sec::authz::{self, Scope, Ability};

mod entries;

pub fn build(_state: &state::SharedState) -> Router<state::SharedState> {
    Router::new()
        .route("/", get(retrieve_journals)
            .post(create_journal))
        .route("/new", get(retrieve_journal))
        .route("/:journals_id", get(retrieve_journal)
            .patch(update_journal))
        .route("/:journals_id/entries", get(entries::retrieve_entries)
            .post(entries::create_entry))
        .route("/:journals_id/entries/new", get(entries::retrieve_entry))
        .route("/:journals_id/entries/:entries_id", get(entries::retrieve_entry)
            .patch(entries::update_entry)
            .delete(entries::delete_entry))
        .route("/:journals_id/entries/:entries_id/:file_entry_id", get(entries::files::retrieve_file)
            .put(entries::files::upload_file))
}

#[derive(Debug, Serialize)]
pub struct JournalPartial {
    pub id: JournalId,
    pub uid: JournalUid,
    pub users_id: UserId,
    pub name: String,
    pub description: Option<String>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

async fn retrieve_journals(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator!(
        &conn,
        &headers,
        Some(uri.clone())
    );

    macros::res_if_html!(state.templates(), &headers);

    let perm_check = authz::has_permission(
        &conn,
        initiator.user.id,
        Scope::Journals,
        Ability::Read
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let params: db::ParamsArray<'_, 1> = [&initiator.user.id];
    let journals = conn.query_raw(
        "\
        with search_journals as ( \
            select * \
            from journals \
            where journals.users_id = $1 \
        ) \
        select search_journals.id, \
               search_journals.uid, \
               search_journals.users_id, \
               search_journals.name, \
               search_journals.description, \
               search_journals.created, \
               search_journals.updated \
        from search_journals \
        order by search_journals.name",
        params
    )
        .await
        .context("failed to retrieve journals")?;

    futures::pin_mut!(journals);

    let mut found = Vec::new();

    while let Some(try_record) = journals.next().await {
        let record = try_record.context("failed to retrieve journal")?;

        found.push(JournalPartial {
            id: record.get(0),
            uid: record.get(1),
            users_id: record.get(2),
            name: record.get(3),
            description: record.get(4),
            created: record.get(5),
            updated: record.get(6),
        });
    }

    Ok(body::Json(found).into_response())
}

#[derive(Debug, Deserialize)]
pub struct MaybeJournalPath {
    journals_id: Option<JournalId>,
}

#[derive(Debug, Deserialize)]
pub struct JournalPath {
    journals_id: JournalId
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

#[derive(Debug, Serialize)]
pub struct JournalFull {
    pub id: JournalId,
    pub uid: JournalUid,
    pub users_id: UserId,
    pub name: String,
    pub description: Option<String>,
    pub custom_fields: Vec<CustomFieldFull>,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
}

async fn retrieve_journal(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
    Path(MaybeJournalPath { journals_id }): Path<MaybeJournalPath>,
) -> Result<Response, error::Error> {
    macros::res_if_html!(state.templates(), &headers);

    let Some(journals_id) = journals_id else {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    let conn = state.db_conn().await?;

    let initiator = macros::require_initiator!(&conn, &headers, Some(uri));

    let perm_check = authz::has_permission(
        &conn,
        initiator.user.id,
        Scope::Journals,
        Ability::Read
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = Journal::retrieve_id(&conn, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve journal")?;

    let Some(journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    let mut custom_fields = Vec::new();
    let fields = CustomField::retrieve_journal_stream(&conn, &journals_id)
        .await
        .context("failed to retrieve custom fields")?;

    futures::pin_mut!(fields);

    while let Some(try_record) = fields.next().await {
        let record = try_record.context("failed to retrieve custom field record")?;

        custom_fields.push(CustomFieldFull {
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

    Ok(body::Json(JournalFull {
        id: journal.id,
        uid: journal.uid,
        users_id: journal.users_id,
        name: journal.name,
        description: journal.description,
        custom_fields,
        created: journal.created,
        updated: journal.updated,
    }).into_response())
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
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum NewJournalResult {
    NameExists,
    DuplicateCustomFields {
        duplicates: Vec<String>,
    },
    Created(JournalFull)
}

async fn create_journal(
    state: state::SharedState,
    headers: HeaderMap,
    body::Json(json): body::Json<NewJournal>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator!(&transaction, &headers, None::<Uri>);

    let perm_check = authz::has_permission(
        &transaction,
        initiator.user.id,
        Scope::Journals,
        Ability::Create
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let mut options = Journal::create_options(initiator.user.id, json.name);

    if let Some(description) = json.description {
        options = options.description(description);
    }

    let result = Journal::create(&transaction, options).await;

    let journal = match result {
        Ok(journal) => journal,
        Err(err) => match err {
            JournalCreateError::NameExists => return Ok((
                StatusCode::BAD_REQUEST,
                body::Json(NewJournalResult::NameExists)
            ).into_response()),
            JournalCreateError::UserNotFound => return Err(
                error::Error::context("specified user does not exist")
            ),
            JournalCreateError::Db(err) => return Err(
                error::Error::context_source(
                    "failed to create journal",
                    err
                )
            ),
        }
    };

    let (custom_fields, duplicates) = create_custom_fields(
        &transaction, &journal, json.custom_fields
    ).await?;

    if !duplicates.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(NewJournalResult::DuplicateCustomFields {
                duplicates
            })
        ).into_response());
    }

    let journal_dir = state.storage()
        .journal_dir(&journal);

    let root_dir = journal_dir.create_root_dir()
        .await
        .context("failed to create root journal directory")?;

    let files_dir = match journal_dir.create_files_dir().await {
        Ok(files) => files,
        Err(err) => {
            if let Err(root_err) = tokio::fs::remove_dir(&root_dir).await {
                error::log_prefix_error(
                    "failed to remove journal root dir",
                    &root_err
                );
            }

            return Err(error::Error::context_source("failed to create journal files dir", err));
        }
    };

    if let Err(err) = transaction.commit().await {
        if let Err(files_err) = tokio::fs::remove_dir(&files_dir).await {
            error::log_prefix_error(
                "failed to remove journal files dir",
                &files_err
            );
        } else if let Err(root_err) = tokio::fs::remove_dir(&root_dir).await {
            error::log_prefix_error(
                "failed to remove journal root dir",
                &root_err
            );
        }

        return Err(error::Error::context_source(
            "failed to commit transaction",
            err
        ));
    }

    Ok(body::Json(NewJournalResult::Created(JournalFull {
        id: journal.id,
        uid: journal.uid,
        users_id: journal.users_id,
        name: journal.name,
        description: journal.description,
        custom_fields,
        created: journal.created,
        updated: journal.updated,
    })).into_response())
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
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum UpdateJournalResult {
    NameExists,
    CustomFieldNotFound {
        ids: Vec<CustomFieldId>,
    },
    DuplicateCustomFields {
        duplicates: Vec<String>,
    },
    Updated(JournalFull),
}

async fn update_journal(
    state: state::SharedState,
    headers: HeaderMap,
    Path(JournalPath { journals_id }): Path<JournalPath>,
    body::Json(json): body::Json<UpdateJournal>,
) -> Result<Response, error::Error> {
    let mut conn = state.db_conn().await?;
    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let initiator = macros::require_initiator!(&transaction, &headers, None::<Uri>);

    let perm_check = authz::has_permission(
        &transaction,
        initiator.user.id,
        Scope::Journals,
        Ability::Update
    )
        .await
        .context("failed to retrieve permission for user")?;

    if !perm_check {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let result = Journal::retrieve_id(&transaction, &journals_id, &initiator.user.id)
        .await
        .context("failed to retrieve journal")?;

    let Some(mut journal) = result else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };

    journal.name = json.name;
    journal.description = json.description;
    journal.updated = Some(Utc::now());

    if let Err(err) = journal.update(&transaction).await {
        match err {
            JournalUpdateError::NameExists => return Ok((
                StatusCode::BAD_REQUEST,
                body::Json(UpdateJournalResult::NameExists)
            ).into_response()),
            JournalUpdateError::NotFound => return Err(
                error::Error::context(
                    "attempted to update journal that no longer exists"
                )
            ),
            JournalUpdateError::Db(err) => return Err(
                error::Error::context_source(
                    "failed to update journal",
                    err
                )
            )
        }
    }

    let UpdateResults {valid, not_found, duplicates} = update_custom_fields(
        &transaction,
        &journal,
        json.custom_fields,
    ).await?;

    if !duplicates.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdateJournalResult::DuplicateCustomFields {
                duplicates
            })
        ).into_response());
    }

    if !not_found.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            body::Json(UpdateJournalResult::CustomFieldNotFound {
                ids: not_found
            })
        ).into_response());
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(body::Json(UpdateJournalResult::Updated(JournalFull {
        id: journal.id,
        uid: journal.uid,
        users_id: journal.users_id,
        name: journal.name,
        description: journal.description,
        custom_fields: valid,
        created: journal.created,
        updated: journal.updated,
    })).into_response())
}

async fn create_custom_fields(
    conn: &impl db::GenericClient,
    journal: &Journal,
    new_fields: Vec<NewCustomField>
) -> Result<(Vec<CustomFieldFull>, Vec<String>), error::Error> {
    if new_fields.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let created = Utc::now();

    let mut records = Vec::new();
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

        records.push(CustomField {
            id: CustomFieldId::zero(),
            uid: CustomFieldUid::gen(),
            journals_id: journal.id,
            name: field.name,
            order: field.order,
            config: field.config,
            description: field.description,
            created,
            updated: None,
        });
    }

    if !duplicates.is_empty() {
        return Ok((Vec::new(), duplicates));
    }

    let rtn = insert_custom_fields(conn, records).await?;

    Ok((rtn, Vec::new()))
}

struct UpdateResults {
    valid: Vec<CustomFieldFull>,
    not_found: Vec<CustomFieldId>,
    duplicates: Vec<String>,
}

async fn update_custom_fields(
    conn: &impl db::GenericClient,
    journal: &Journal,
    update_fields: Vec<UpdateCustomField>,
) -> Result<UpdateResults, error::Error> {
    let mut existing: HashMap<CustomFieldId, CustomField> = HashMap::new();
    let stream = CustomField::retrieve_journal_stream(conn, &journal.id)
        .await
        .context("failed to retrieve current custom fields")?;

    futures::pin_mut!(stream);

    while let Some(try_record) = stream.next().await {
        let record = try_record.context("failed to retrieve custom_field record")?;

        tracing::debug!("existing record: {record:#?}");

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

                insert_records.push(CustomField {
                    id: CustomFieldId::zero(),
                    uid: CustomFieldUid::gen(),
                    journals_id: journal.id,
                    name: new_field.name,
                    order: new_field.order,
                    config: new_field.config,
                    description: new_field.description,
                    created,
                    updated: None,
                });
            }
        }
    }

    if !duplicates.is_empty() || !not_found.is_empty() {
        return Ok(UpdateResults {
            valid: Vec::new(),
            not_found,
            duplicates,
        });
    }

    if !insert_records.is_empty() {
        rtn.extend(insert_custom_fields(conn, insert_records).await?);
    }

    {
        let mut await_list = futures::stream::FuturesUnordered::new();

        for existing in &update_records {
            let params: db::ParamsArray<'_, 5> = [
                &existing.id,
                &existing.name,
                &existing.order,
                &existing.description,
                &existing.updated,
            ];

            await_list.push(conn.execute_raw(
                "\
                update custom_fields \
                set name = $2, \
                    \"order\" = $3, \
                    description = $4, \
                    updated = $5 \
                where id = $1",
                params
            ));
        }

        let mut failed = false;

        while let Some(result) = await_list.next().await {
            if let Err(err) = result {
                error::log_prefix_error("failed to update custom_field", &err);

                failed = true;
            }
        }

        if failed {
            return Err(error::Error::context("error when updating custom_fields"));
        }
    }

    rtn.extend(update_records.into_iter().map(|record| CustomFieldFull {
        id: record.id,
        uid: record.uid,
        name: record.name,
        order: record.order,
        config: record.config,
        description: record.description,
        created: record.created,
        updated: record.updated,
    }));

    if !existing.is_empty() {
        let ids: Vec<CustomFieldId> = existing.into_keys()
            .collect();

        tracing::debug!("deleting ids: {ids:#?}");

        conn.execute(
            "delete from custom_fields where id = any($1)",
            &[&ids]
        )
            .await
            .context("failed to delete custom fields")?;
    }

    Ok(UpdateResults {
        valid: rtn,
        not_found: Vec::new(),
        duplicates: Vec::new(),
    })
}

async fn insert_custom_fields(
    conn: &impl db::GenericClient,
    records: Vec<CustomField>
) -> Result<Vec<CustomFieldFull>, error::Error> {
    let mut rtn = Vec::with_capacity(records.len());
    let mut query = String::from(
        "insert into custom_fields (uid, journals_id, name, \"order\", config, description, created) values"
    );
    let mut params: db::ParamsVec<'_> = Vec::new();

    for (index, field) in records.iter().enumerate() {
        if index > 0 {
            query.push_str(", ");
        }

        let s = format!(
            "(${}, ${}, ${}, ${}, ${}, ${}, ${})",
            db::push_param(&mut params, &field.uid),
            db::push_param(&mut params, &field.journals_id),
            db::push_param(&mut params, &field.name),
            db::push_param(&mut params, &field.order),
            db::push_param(&mut params, &field.config),
            db::push_param(&mut params, &field.description),
            db::push_param(&mut params, &field.created),
        );

        query.push_str(&s);
    }

    query.push_str(" returning id");

    let results = conn.query_raw(&query, params)
        .await
        .context("failed to insert new custom fields")?;

    futures::pin_mut!(results);

    let mut zipped = results.zip(futures::stream::iter(records));

    while let Some((try_record, field)) = zipped.next().await {
        let record = try_record.context("failed to retrieve custom field record")?;
        let id = record.get(0);

        rtn.push(CustomFieldFull {
            id,
            uid: field.uid,
            name: field.name,
            order: field.order,
            config: field.config,
            description: field.description,
            created: field.created,
            updated: field.updated,
        });
    }

    Ok(rtn)
}