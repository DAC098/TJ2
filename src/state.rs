use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::config;
use crate::db;
use crate::db::ids::{JournalId, FileEntryId};
use crate::error::{self, Context};
use crate::journal::{Journal, JournalDir};
use crate::templates;

#[derive(Debug, Clone)]
pub struct SharedState(Arc<State>);

impl SharedState {
    pub async fn new(config: &config::Config) -> Result<Self, error::Error> {
        let db_pool = db::from_config(config).await?;
        let templates = templates::initialize(config)?;

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

    pub async fn db_conn(&self) -> Result<db::Object, error::Error> {
        self.0.db_pool.get()
            .await
            .context("failed to retrieve database connection")
    }
}

#[async_trait]
impl FromRequestParts<SharedState> for SharedState {
    type Rejection = Infallible;

    async fn from_request_parts(_: &mut Parts, state: &SharedState) -> Result<Self, Self::Rejection> {
        Ok(state.clone())
    }
}

#[derive(Debug)]
pub struct State {
    db_pool: db::Pool,
    assets: Assets,
    storage: Storage,
    templates: tera::Tera,
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

#[derive(Debug)]
pub struct Storage {
    path: PathBuf
}

impl Storage {
    pub fn journal_dir(&self, journal: &Journal) -> JournalDir {
        JournalDir::new(&self.path, journal)
    }

    pub fn journal_file_entry(
        &self,
        journal_id: JournalId,
        file_entry_id: FileEntryId
    ) -> PathBuf {
        self.path.join(format!("journals/{journal_id}/files/{file_entry_id}.file"))
    }
}
