use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::config;
use crate::error;
use crate::db;
use crate::templates;

#[derive(Debug, Clone)]
pub struct SharedState(Arc<State>);

impl SharedState {
    pub async fn new(config: &config::Config) -> Result<Self, error::Error> {
        let db_pool = db::connect(config).await?;
        let templates = templates::initialize(config)?;

        Ok(SharedState(Arc::new(State {
            db_pool,
            assets: Assets {
                files: config.settings.assets.files.clone(),
                directories: config.settings.assets.directories.clone(),
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
    db_pool: db::DbPool,
    assets: Assets,
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
                tracing::debug!("found asset directory: {key}");

                return Some((dir, stripped));
            }
        }

        None
    }
}
