use std::fmt::Debug;
use std::str::FromStr;

use serde::{Serialize, Deserialize};
use sqlx::{Connection, ConnectOptions};
use sqlx::sqlite::{
    Sqlite,
    SqlitePool,
    SqlitePoolOptions,
    SqliteConnectOptions,
    SqliteJournalMode,
    SqliteConnection,
};

use crate::error::{Error, Context};
use crate::config::{Config, meta::get_cwd};
use crate::path::metadata;
use crate::sec::authz::{
    Scope, Ability, Role,
    create_permissions,
    assign_user_role,
};
use crate::sec::password;
use crate::user::User;

mod test_data;

pub mod ids;

pub type Db = Sqlite;
pub type DbPool = SqlitePool;
pub type DbConn = SqliteConnection;

pub async fn connect(config: &Config) -> Result<SqlitePool, Error> {
    let db_path = config.settings.data.join("database.db");
    let db_url = format!("sqlite://{}", db_path.display());

    tracing::debug!("db_url: {db_url}");

    let connect_options = SqliteConnectOptions::from_str(&db_url)
        .context("invalid sqlite url")?
        .journal_mode(SqliteJournalMode::Truncate);

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
            } else if let Err(err) = std::fs::remove_file(&db_path) {
                tracing::error!("failed to remove database.db: {err:#?}");
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

    let maybe_admin = User::retrieve_username(conn, "admin")
        .await
        .context("failed to check if admin user was found")?;

    if maybe_admin.is_none() {
        let mut rng = rand::thread_rng();
        let admin = create_admin_user(conn).await?;
        let admin_role = create_default_roles(conn).await?;

        assign_user_role(conn, admin.id, admin_role.id)
            .await
            .context("failed to assign admin to admin role")?;

        test_data::create_journal(conn, &mut rng, admin.id).await?;
        test_data::create(conn, &mut rng).await?;
    }

    Ok(())
}

async fn create_admin_user(conn: &mut DbConn) -> Result<User, Error> {
    let hash = password::create("password")
        .context("failed to create admin password")?;

    User::create(conn, "admin", &hash, 0)
        .await
        .context("failed to create admin user")
}

async fn create_default_roles(conn: &mut DbConn) -> Result<Role, Error> {
    let admin_role = Role::create(conn, "admin")
        .await
        .context("failed to create admin role")?;

    let permissions = vec![
        (Scope::Users, vec![
            Ability::Create,
            Ability::Read,
            Ability::Update,
            Ability::Delete
        ]),
        (Scope::Journals, vec![
            Ability::Create,
            Ability::Read,
            Ability::Update,
            Ability::Delete,
        ]),
        (Scope::Entries, vec![
            Ability::Create,
            Ability::Read,
            Ability::Update,
            Ability::Delete,
        ])
    ];

    create_permissions(conn, admin_role.id, permissions)
        .await
        .context("failed to create default permissions")?;

    Ok(admin_role)
}

use tokio_postgres::{Config as PgConfig, NoTls};
use deadpool_postgres::{Manager, ManagerConfig, RecyclingMethod, Object};

pub use deadpool_postgres::{Pool, GenericClient};
pub use tokio_postgres::Error as PgError;
pub use tokio_postgres::types::{self, ToSql};

pub type PgJson<T> = types::Json<T>;

pub type ParamsVec<'a> = Vec<&'a (dyn ToSql + Sync)>;
pub type ParamsArray<'a, const N: usize> = [&'a (dyn ToSql + Sync); N];

pub async fn from_config(config: &Config) -> Result<Pool, Error> {
    let mut pg_config = PgConfig::new();

    pg_config.user(config.settings.db.user.as_str());
    pg_config.host(config.settings.db.host.as_str());
    pg_config.port(config.settings.db.port);
    pg_config.dbname(config.settings.db.dbname.as_str());

    if let Some(password) = &config.settings.db.password {
        pg_config.password(password.as_str());
    }

    let manager_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast
    };

    let manager = Manager::from_config(pg_config, NoTls, manager_config);

    let pool = Pool::builder(manager)
        .max_size(4)
        .build()
        .context("failed to create postgresql connection pool")?;

    check_database(&pool).await?;

    Ok(pool)
}

pub async fn check_database(pool: &Pool) -> Result<(), Error> {
    let mut conn = pool.get()
        .await
        .context("failed to retrieve database connection")?;

    let transaction = conn.transaction()
        .await
        .context("failed to create transaction")?;

    let maybe_admin = User::retrieve_username_pg(&transaction, "admin")
        .await
        .context("failed to check if admin user was found")?;

    if maybe_admin.is_none() {
        let mut rng = rand::thread_rng();
        let admin = create_admin_user_pg(&transaction).await?;
        let admin_role = create_default_roles_pg(&transaction).await?;

        admin_role.assign_user(&transaction, admin.id)
            .await
            .context("failed to assign admin to admin role")?;

        test_data::create_journal_pg(&transaction, &mut rng, admin.id).await?;
        test_data::create_pg(&transaction, &mut rng).await?;
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(())
}

async fn create_admin_user_pg(conn: &impl GenericClient) -> Result<User, Error> {
    let hash = password::create("password")
        .context("failed to create admin password")?;

    User::create_pg(conn, "admin", &hash, 0)
        .await
        .context("failed to create admin user")
}

async fn create_default_roles_pg(conn: &impl GenericClient) -> Result<Role, Error> {
    let admin_role = Role::create_pg(conn, "admin")
        .await
        .context("failed to create admin role")?;

    let permissions = vec![
        (Scope::Users, vec![
            Ability::Create,
            Ability::Read,
            Ability::Update,
            Ability::Delete
        ]),
        (Scope::Journals, vec![
            Ability::Create,
            Ability::Read,
            Ability::Update,
            Ability::Delete,
        ]),
        (Scope::Entries, vec![
            Ability::Create,
            Ability::Read,
            Ability::Update,
            Ability::Delete,
        ])
    ];

    admin_role.assign_permissions(conn, &permissions)
        .await
        .context("failed to create default permissions")?;

    Ok(admin_role)
}

pub fn push_param<'a, T>(params: &mut ParamsVec<'a>, v: &'a T) -> usize
where
    T: ToSql + Sync
{
    params.push(v);
    params.len()
}

#[inline]
pub fn de_from_sql<'a, T>(value: PgJson<T>) -> T
where
    T: Deserialize<'a>
{
    value.0
}

#[inline]
pub fn ser_to_sql<'a, T> (value: &'a T) -> PgJson<&'a T>
where
    T: Serialize + Debug
{
    types::Json(value)
}
