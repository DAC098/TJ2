use anyhow::Context;
use clap::{Args, Subcommand};
use tokio::task::JoinHandle;
use tokio_postgres::Client;

mod migrate;

#[derive(Debug, Args)]
pub struct DbArg {
    #[command(flatten)]
    conn: DbConn,

    #[command(subcommand)]
    cmd: DbCmd,
}

#[derive(Debug, Subcommand)]
enum DbCmd {
    /// runs the initialization process for creating the database
    Init(InitArgs),

    /// commands specific to database migrations
    Migrate(migrate::MigrateArgs),
}

#[derive(Debug, Args)]
pub struct DbConn {
    /// user to connect to database
    #[arg(short = 'U', long, default_value = "tj2")]
    user: String,

    /// prompt for a password
    #[arg(long)]
    password: bool,

    /// port to connect to database
    #[arg(short, long, default_value = "5432")]
    port: u16,

    /// host of the database
    #[arg(short = 'H', long, default_value = "localhost")]
    host: String,

    /// name of the database
    #[arg(short = 'D', long, default_value = "tj2")]
    dbname: String,
}

impl DbConn {
    async fn create(&self) -> anyhow::Result<(Client, JoinHandle<()>)> {
        let mut config = tokio_postgres::Config::new();
        config.user(&self.user);
        config.host(&self.host);
        config.port(self.port);
        config.dbname(&self.dbname);

        if self.password {
            let prompt = format!("{}'s password: ", self.user);

            let password = rpassword::prompt_password(prompt)?;

            config.password(&password);
        }

        let (client, conn) = config.connect(tokio_postgres::NoTls).await?;

        let task = tokio::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("database connection error: {e:#?}");
            }
        });

        Ok((client, task))
    }
}

pub async fn handle(db: DbArg) -> anyhow::Result<()> {
    let (mut client, conn_task) = db.conn.create().await?;

    match db.cmd {
        DbCmd::Init(args) => handle_init(&mut client, args).await?,
        DbCmd::Migrate(args) => migrate::handle(&mut client, args).await?,
    }

    std::mem::drop(client);

    conn_task.await?;

    Ok(())
}

/// creates the database for the server if it does not exist.
///
/// the database connection arguments will need to have high level permissions
/// as this will create the database and user that the server will use
#[derive(Debug, Args)]
pub struct InitArgs {
    /// name of the database to create
    #[arg(long, default_value = "tj2")]
    database: String,

    /// optionally create a user
    ///
    /// user will be assigned access privilages
    #[arg(long)]
    username: Option<String>,

    /// specifies if the user has a an encrypted password
    #[arg(long)]
    encrypted: bool,
}

async fn handle_init(
    client: &mut Client,
    InitArgs {
        database,
        username,
        encrypted,
    }: InitArgs,
) -> anyhow::Result<()> {
    let query = format!("create database {database} with encoding 'UTF8'");

    client
        .execute(&query, &[])
        .await
        .context("failed to create database")?;

    if let Some(username) = username {
        let transaction = client.transaction().await?;

        let query = {
            let prompt = format!("{username}'s password: ");
            let password = rpassword::prompt_password(prompt)?;

            let privileges = "nosuperuser nocreatedb nocreaterole noinherit";

            if encrypted {
                format!("create role {username} {privileges} login encrypted password '{password}'")
            } else {
                format!("create role {username} {privileges} login password '{password}'")
            }
        };

        transaction
            .execute(&query, &[])
            .await
            .context("failed to creaet database user")?;

        let query = format!("grant connect on database {database} to {username}");

        transaction
            .execute(&query, &[])
            .await
            .context("failed to grant connect privilege on database")?;

        let query = format!("grant pg_read_all_data to {username}");

        transaction
            .execute(&query, &[])
            .await
            .context("failed to grant read privileges")?;

        let query = format!("grant pg_write_all_data to {username}");

        transaction
            .execute(&query, &[])
            .await
            .context("failed to grant write privileges")?;

        transaction.commit().await?;
    }

    Ok(())
}
