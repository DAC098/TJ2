use std::fmt::Debug;

use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use deadpool_postgres::{Manager, ManagerConfig, RecyclingMethod};
use serde::{Serialize, Deserialize};
use tokio_postgres::{Config as PgConfig, NoTls};

use crate::config::Config;
use crate::error::{Error, Context};
use crate::sec::authz::{Scope, Ability, Role};
use crate::sec::password;
use crate::state;
use crate::user::User;

pub use deadpool_postgres::{Pool, GenericClient, Object};
pub use tokio_postgres::Error as PgError;
pub use tokio_postgres::types::{self, ToSql};

mod test_data;

pub mod ids;

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

    let maybe_admin = User::retrieve_username(&transaction, "admin")
        .await
        .context("failed to check if admin user was found")?;

    if maybe_admin.is_none() {
        let mut rng = rand::thread_rng();
        let admin = create_admin_user(&transaction).await?;
        let admin_role = create_default_roles(&transaction).await?;

        admin_role.assign_user(&transaction, admin.id)
            .await
            .context("failed to assign admin to admin role")?;

        test_data::create_journal(&transaction, &mut rng, admin.id).await?;
        test_data::create(&transaction, &mut rng).await?;
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(())
}

async fn create_admin_user(conn: &impl GenericClient) -> Result<User, Error> {
    let hash = password::create("password")
        .context("failed to create admin password")?;

    User::create(conn, "admin", &hash, 0)
        .await
        .context("failed to create admin user")
}

async fn create_default_roles(conn: &impl GenericClient) -> Result<Role, Error> {
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

pub struct Conn(pub Object);

#[async_trait]
impl FromRequestParts<state::SharedState> for Conn {
    type Rejection = Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &state::SharedState
    ) -> Result<Self, Self::Rejection> {
        let conn = state.db()
            .get()
            .await
            .context("failed to retrieve database connection")?;

        Ok(Self(conn))
    }
}
