use std::str::FromStr;

use sqlx::ConnectOptions;
use sqlx::sqlite::{
    SqlitePool,
    SqlitePoolOptions,
    SqliteConnectOptions,
    SqliteJournalMode,
    SqliteConnection,
};

use crate::error::{Error, Context};
use crate::config::{Config, meta::get_cwd};
use crate::path::metadata;

pub type DbPool = SqlitePool;
pub type DbConn = SqliteConnection;

pub async fn connect(config: &Config) -> Result<SqlitePool, Error> {
    let db_path = config.settings.data.join("database.db");
    let db_url = format!("sqlite://{}", db_path.display());

    tracing::debug!("db_url: {db_url}");

    let connect_options = SqliteConnectOptions::from_str(&db_url)
        .context("invalid sqlite url")?
        .journal_mode(SqliteJournalMode::Wal);

    if let Some(meta) = metadata(&db_path).context("failed to retrieve metadata for db file")? {
        if !meta.is_file() {
            return Err(Error::context("sqlite database.db is not a file"));
        }
    } else {
        tracing::debug!("sqlite database file does not exist");

        let mut conn = connect_options.clone()
            .create_if_missing(true)
            .connect()
            .await
            .context("failed to connect to sqlite database")?;

        init_database(&mut conn).await?;
    }

    SqlitePoolOptions::new()
        .connect_with(connect_options)
        .await
        .context("failed to create sqlite pool")
}

async fn init_database(conn: &mut SqliteConnection) -> Result<(), Error> {
    let init_path = get_cwd()?.join("db/sqlite/init.sql");
    let init_sql = tokio::fs::read_to_string(&init_path)
        .await
        .context("failed to open sqlite init script")?;

    for statement in init_sql.split(';') {
        let trimmed = statement.trim();

        if trimmed.is_empty() {
            break;
        }

        tracing::debug!("executing: \"{trimmed}\"");

        sqlx::query(trimmed)
            .execute(&mut *conn)
            .await
            .context("failed to run sql query")?;
    }

    Ok(())
}
