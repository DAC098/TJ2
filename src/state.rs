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
            db_pool
        })))
    }
}

#[derive(Debug)]
pub struct State {
    db_pool: db::DbPool,
}
