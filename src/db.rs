use std::fmt::Debug;

use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use deadpool_postgres::{Manager, ManagerConfig, RecyclingMethod};
use serde::{Serialize, Deserialize};
use tokio_postgres::{Config as PgConfig, NoTls};
use tokio_postgres::error::SqlState;

use crate::config::Config;
use crate::error::{Error, Context};
use crate::sec::authz::{Scope, Ability, Role};
use crate::sec::password;
use crate::state;
use crate::user::User;

pub use deadpool_postgres::{Pool, GenericClient, Object, Transaction};
pub use tokio_postgres::Error as PgError;
pub use tokio_postgres::types::{self, ToSql};

mod test_data;

pub mod ids;

/// type alias for tokio_postgres::types::Json
pub type PgJson<T> = types::Json<T>;

/// type alias for creating a Vec of ToSql references
pub type ParamsVec<'a> = Vec<&'a (dyn ToSql + Sync)>;

/// type alias for creating a fixed size array of ToSql references
pub type ParamsArray<'a, const N: usize> = [&'a (dyn ToSql + Sync); N];

/// creates the postgres database connection pool
///
/// the connection pool will be limited for 4
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

/// checks to make sure that the admin account exists in the database with
/// the necessary permissions.
///
/// if the admin account is not found then it will attempt to create the
/// user and role. this is a quick check will assume that if the admin
/// user exists then the role will as well.
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
        let admin = create_admin_user(&transaction)
            .await?
            .context("admin already exists. prior lookup failed")?;
        let admin_role = create_default_roles(&transaction)
            .await?;

        admin_role.assign_user(&transaction, admin.id)
            .await
            .context("failed to assign admin to admin role")?;
    }

    transaction.commit()
        .await
        .context("failed to commit transaction")?;

    Ok(())
}

/// creates the default admin user
async fn create_admin_user(conn: &impl GenericClient) -> Result<Option<User>, Error> {
    let hash = password::create("password")
        .context("failed to create admin password")?;

    User::create(conn, "admin", &hash, 0)
        .await
        .context("failed to create admin user")
}

/// creates the default admin role
async fn create_default_roles(conn: &impl GenericClient) -> Result<Role, Error> {
    let admin_role = Role::create(conn, "admin")
        .await
        .context("failed to create admin role")?
        .context("admin role already exists")?;

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
        ]),
        (Scope::Groups, vec![
            Ability::Create,
            Ability::Read,
            Ability::Update,
            Ability::Delete,
        ]),
        (Scope::Roles, vec![
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

/// generates test data for the server to use for testing purposes
pub async fn gen_test_data(state: &state::SharedState) -> Result<(), Error> {
    let mut rng = rand::thread_rng();
    let mut conn = state.db_conn().await?;

    let transaction = conn.transaction()
        .await
        .context("failed to create database transaction")?;

    let maybe_admin = User::retrieve_username(&transaction, "admin")
        .await
        .context("failed to check if admin user was found")?;

    if let Some(admin) = maybe_admin {
        let check = transaction.execute(
            "select * from journals where id = $1",
            &[&admin.id]
        )
            .await
            .context("failed to retrieve journals for admin")?;

        if check == 0 {
            test_data::create_journal(
                state,
                &transaction,
                &mut rng,
                admin.id
            ).await?;
        }
    }

    test_data::create(state, &transaction, &mut rng).await?;

    transaction.commit()
        .await
        .context("failed to commit transaction for test data")?;

    Ok(())
}

/// helper method to push a new ToSql reference and returning the new length
///
/// used for query parameters when dynmaically creating sql queries
pub fn push_param<'a, T>(params: &mut ParamsVec<'a>, v: &'a T) -> usize
where
    T: ToSql + Sync
{
    params.push(v);
    params.len()
}

/// helper method for converting a database value to the serde deserializable
/// object
#[inline]
pub fn de_from_sql<'a, T>(value: PgJson<T>) -> T
where
    T: Deserialize<'a>
{
    value.0
}

/// helper method for converting a serde serializable object to a database
/// value
#[inline]
pub fn ser_to_sql<'a, T> (value: &'a T) -> PgJson<&'a T>
where
    T: Serialize + Debug
{
    types::Json(value)
}

/// helper enum for determing if the database error is one of the variants
/// specified
pub enum ErrorKind<'a> {
    /// in the event that the database error is a UNIQUE_VIOLOATION and
    /// provides the constraint that caused the violation
    Unique(&'a str),

    /// in the vent that the database error is a FOREIGN_KEY_VIOLATION and
    /// provides the constraint that caused the violation
    ForeignKey(&'a str),
}

impl<'a> ErrorKind<'a> {
    /// checks to see if the database error fills one of the variants
    pub fn check(error: &'a PgError) -> Option<Self> {
        let Some(db_error) = error.as_db_error() else {
            return None;
        };

        match *db_error.code() {
            SqlState::UNIQUE_VIOLATION => if let Some(name) = db_error.constraint() {
                Some(Self::Unique(name))
            } else {
                None
            }
            SqlState::FOREIGN_KEY_VIOLATION => if let Some(name) = db_error.constraint() {
                Some(Self::ForeignKey(name))
            } else {
                None
            }
            _ => None
        }
    }
}

// could directly implement FromRequestParts for Object
/// allows for getting access to a database connection without having to
/// manually handle the errors
pub struct Conn(pub Object);

impl Conn {
    /// attempts to retrieve a database transaction from the current
    /// connection
    pub async fn transaction(&mut self) -> Result<Transaction<'_>, Error> {
        self.0.transaction()
            .await
            .context("failed to create transaction")
    }
}

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

#[async_trait]
impl FromRequestParts<()> for Conn {
    type Rejection = Error;

    async fn from_request_parts(
        _parts: &mut Parts,
        _state: &()
    ) -> Result<Self, Self::Rejection> {
        Err(Error::context("no state"))
    }
}
