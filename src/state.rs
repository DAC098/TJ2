use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::config;
use crate::db;
use crate::db::ids::{FileEntryId, JournalId, UserId};
use crate::error::{self, Context};
use crate::journal::JournalDir;
use crate::sec::authn::session::ApiSessionToken;
use crate::sec::otp::Totp;
use crate::sec::pki::Data;
use crate::templates;
use crate::user::UserDir;

#[derive(Debug, Clone)]
pub struct SharedState(Arc<State>);

impl SharedState {
    pub async fn new(config: &config::Config) -> Result<Self, error::Error> {
        let db_pool = db::from_config(config).await?;
        let templates = templates::initialize(config)?;
        let security = Security::new();

        Ok(SharedState(Arc::new(State {
            db_pool,
            assets: Assets {
                files: config.settings.assets.files.clone(),
                directories: config.settings.assets.directories.clone(),
            },
            storage: Storage {
                path: config.settings.storage.clone(),
            },
            templates,
            security,
        })))
    }

    pub fn assets(&self) -> &Assets {
        &self.0.assets
    }

    pub fn templates(&self) -> &tera::Tera {
        &self.0.templates
    }

    pub fn db(&self) -> &db::Pool {
        &self.0.db_pool
    }

    pub fn storage(&self) -> &Storage {
        &self.0.storage
    }

    pub fn security(&self) -> &Security {
        &self.0.security
    }

    pub async fn db_conn(&self) -> Result<db::Object, error::Error> {
        self.0
            .db_pool
            .get()
            .await
            .context("failed to retrieve database connection")
    }
}

#[async_trait]
impl FromRequestParts<SharedState> for SharedState {
    type Rejection = Infallible;

    async fn from_request_parts(
        _: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        Ok(state.clone())
    }
}

#[async_trait]
impl FromRequestParts<()> for SharedState {
    type Rejection = Infallible;

    async fn from_request_parts(
        _: &mut Parts,
        _: &()
    ) -> Result<Self, Self::Rejection> {
        panic!("no shared state available");
    }
}

#[derive(Debug)]
pub struct State {
    db_pool: db::Pool,
    assets: Assets,
    storage: Storage,
    templates: tera::Tera,
    security: Security,
}

#[derive(Debug)]
pub struct Assets {
    files: HashMap<String, PathBuf>,
    directories: HashMap<String, PathBuf>,
}

impl Assets {
    pub fn get_file(&self, uri: &str) -> Option<&Path> {
        if let Some(found) = self.files.get(uri) {
            Some(found)
        } else {
            None
        }
    }

    pub fn get_dir<'a>(&self, uri: &'a str) -> Option<(&Path, &'a str)> {
        for (key, dir) in &self.directories {
            if let Some(stripped) = uri.strip_prefix(key) {
                //tracing::debug!("found asset directory: {key}");

                return Some((dir, stripped));
            }
        }

        None
    }
}

#[derive(Debug, Clone)]
pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn user_dir(&self, users_id: UserId) -> UserDir {
        UserDir::new(&self.path, users_id)
    }

    pub fn journal_dir(&self, journals_id: JournalId) -> JournalDir {
        JournalDir::new(&self.path, journals_id)
    }

    pub fn journal_file_entry(
        &self,
        journals_id: JournalId,
        file_entry_id: FileEntryId,
    ) -> PathBuf {
        self.path
            .join(format!("journals/{journals_id}/files/{file_entry_id}.file"))
    }
}

#[async_trait]
impl FromRequestParts<SharedState> for Storage {
    type Rejection = Infallible;

    async fn from_request_parts(
        _: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        Ok(state.0.storage.clone())
    }
}

#[derive(Debug)]
pub struct Security {
    pub vetting: Vetting,
    pub authn: Authn,
}

#[derive(Debug)]
pub struct Vetting {
    pub totp: moka::sync::Cache<UserId, Totp>,
}

#[derive(Debug)]
pub struct Authn {
    pub api: moka::sync::Cache<ApiSessionToken, Data>,
}

impl Security {
    fn new() -> Self {
        Self {
            vetting: Vetting::new(),
            authn: Authn::new(),
        }
    }
}

impl Vetting {
    fn new() -> Self {
        let totp = moka::sync::Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(3 * 60))
            .build();

        Self { totp }
    }
}

impl Authn {
    fn new() -> Self {
        let api = moka::sync::Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(60))
            .build();

        Self { api }
    }
}
