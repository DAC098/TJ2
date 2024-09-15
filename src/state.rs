use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::ops::Deref;
use std::sync::Arc;

use crate::config;
use crate::db;
use crate::error;

#[derive(Debug, Clone)]
pub struct SharedState(Arc<State>);

impl SharedState {
    pub async fn new(config: &config::Config) -> Result<Self, error::Error> {
        let db_pool = db::connect(config).await?;

        Ok(SharedState(Arc::new(State {
            db_pool,
            assets: Assets {
                files: config.settings.assets.files.clone(),
                directories: config.settings.assets.directories.clone(),
            }
        })))
    }
}

impl Deref for SharedState {
    type Target = State;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct State {
    db_pool: db::DbPool,
    assets: Assets,
}

impl State {
    pub fn assets(&self) -> &Assets {
        &self.assets
    }
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
