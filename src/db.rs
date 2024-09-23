use std::str::FromStr;

use sqlx::{Connection, ConnectOptions};
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

        if let Err(err) = init_database(&mut conn).await {
            if let Err(err) = conn.close().await {
                tracing::error!("failed to close connection to database: {err:#?}");
            } else {
                if let Err(err) = std::fs::remove_file(&db_path) {
                    tracing::error!("failed to remove database.db: {err:#?}");
                }
            }

            return Err(err);
        }
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

    let maybe_found = sqlx::query("select * from users where username = 'admin'")
        .fetch_optional(&mut *conn)
        .await
        .context("failed to check if admin user was found")?;

    if maybe_found.is_none() {
        create_admin_user(conn).await?;
    }

    Ok(())
}

async fn create_admin_user(conn: &mut SqliteConnection) -> Result<(), Error> {
    use argon2::Argon2;
    use argon2::password_hash::{PasswordHasher, SaltString};
    use argon2::password_hash::rand_core::OsRng;

    let password = b"password";
    let salt = SaltString::generate(&mut OsRng);

    let config = Argon2::default();
    let password_hash = match config.hash_password(password, &salt) {
        Ok(hashed) => hashed.to_string(),
        Err(err) => {
            tracing::debug!("argon2 hash_password error: {err:#?}");

            return Err(Error::context("failed to hash admin password"));
        }
    };

    sqlx::query(
        "\
        insert into users (uid, username, password, version) values \
        (?1, ?2, ?3, ?4)"
    )
        .bind(&"")
        .bind(&"admin")
        .bind(&password_hash)
        .bind(&0)
        .execute(&mut *conn)
        .await
        .context("failed to create admin user")?;

    Ok(())
}
