use std::path::PathBuf;

use anyhow::{Result, Context};
use clap::{Parser, Args, Subcommand};

const DEFAULT_SQL: &'static str = include_str!("../db/postgres/init.sql");

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    match args.cmd {
        Cmd::Init(init_args) => init(init_args).await,
        Cmd::Fs(fs_args) => handle_fs(fs_args).await,
        Cmd::Db(db_args) => handle_db(db_args).await,
    }
}

#[derive(Debug, Parser)]
struct CliArgs {
    #[command(subcommand)]
    cmd: Cmd
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// initializes the necessary directories and database for the server
    Init(Init),

    /// commands related to file system management
    Fs(Fs),

    /// commands related to database management
    Db(Db)
}

#[derive(Debug, Args)]
struct PostgresArgs {
    /// the username to use when attempting to access the database
    #[arg(default_value("postgres"))]
    user: String,

    /// the hostname of the machine to connect to
    #[arg(default_value("localhost"))]
    host: String,

    /// the port that the database is listening on
    #[arg(default_value("5432"))]
    port: u16,

    /// name of the database to connect with
    #[arg(default_value("tj2"))]
    dbname: String,
}

#[derive(Debug, Args)]
struct Init {
    #[command(flatten)]
    postgres: PostgresArgs,
}

async fn init(_init: Init) -> Result<()> {
    Ok(())
}

#[derive(Debug, Args)]
struct Fs {
    #[command(subcommand)]
    cmd: FsCmd
}

#[derive(Debug, Subcommand)]
enum FsCmd {
    /// initalizes the file system for use with the server
    Init(FsInit),
}

async fn handle_fs(fs: Fs) -> Result<()> {
    match fs.cmd {
        FsCmd::Init(init_args) => fs_init(init_args).await,
    }
}

#[derive(Debug, Args)]
struct FsInit {
    /// the location to create the data directory
    #[arg(long, default_value("./"))]
    data: PathBuf,

    /// the location to create the storage directory
    #[arg(long, default_value("./"))]
    storage: PathBuf,
}

async fn fs_init(_init: FsInit) -> Result<()> {
    Ok(())
}

#[derive(Debug, Args)]
pub struct Db {
    #[command(subcommand)]
    cmd: DbCmd
}

#[derive(Debug, Subcommand)]
enum DbCmd {
    /// initializes the database for use with the server
    Init(DbInit)
}

async fn handle_db(db: Db) -> Result<()> {
    match db.cmd {
        DbCmd::Init(init_args) => db_init(init_args).await,
    }
}

async fn get_conn(config: tokio_postgres::config::Config) -> Result<(tokio_postgres::Client, tokio::task::JoinHandle<()>)> {
    let (client, connection) = config.connect(tokio_postgres::NoTls)
        .await
        .context("failed to connect to postgres database")?;

    let handle = tokio::spawn(async move {
        if let Err(err) = connection.await {
            eprintln!("tokio postgres connection error: {err}");
        }
    });

    Ok((client, handle))
}

#[derive(Debug, Args)]
struct DbInit {
    #[command(flatten)]
    postgres: PostgresArgs,

    /// overrides the builtin sql script that will be executed on the database
    #[arg(long)]
    file: Option<PathBuf>,

    /// attempts to create the database if it does not exist
    #[arg(long, conflicts_with("recreate"))]
    create: bool,

    /// attempts to drop and re-initialize the database
    #[arg(long, conflicts_with("create"))]
    recreate: bool,
}

async fn db_init(init: DbInit) -> Result<()> {
    let password = {
        let prompt = format!("{} password: ", init.postgres.user);

        rpassword::prompt_password(&prompt)
            .context("failed to retrieve password for db connection")?
    };

    let mut config = tokio_postgres::config::Config::new();
    config.user(init.postgres.user);
    config.password(&password);
    config.host(init.postgres.host);
    config.port(init.postgres.port);

    if init.recreate {
        println!("recreating database");

        let mut rc_config = config.clone();
        rc_config.dbname("postgres");

        let (client, handle) = get_conn(rc_config).await?;

        let query = format!("drop database if exists {}", init.postgres.dbname);

        client.execute(&query, &[])
            .await
            .context("failed to drop database")?;

        let query = format!("create database {}", init.postgres.dbname);

        client.execute(&query, &[])
            .await
            .context("failed to create database")?;

        std::mem::drop(client);

        handle.await
            .context("failed to join db connection")?;
    } else if init.create {
        println!("creating database");

        let mut c_config = config.clone();
        c_config.dbname("postgres");

        let (client, handle) = get_conn(c_config).await?;

        let query = format!(
            "select from pg_database where datname = '{}'",
            init.postgres.dbname
        );

        let result = client.execute(&query, &[])
            .await
            .context("failed to create database")?;

        if result == 0 {
            let query = format!("create database {}", init.postgres.dbname);

            client.execute(&query, &[])
                .await
                .context("failed to create database")?;
        }

        std::mem::drop(client);

        handle.await
            .context("failed to join db connection")?;
    }

    config.dbname(init.postgres.dbname);

    let (client, handle) = get_conn(config).await?;

    let maybe_contents;

    let commands = if let Some(init_file) = init.file {
        maybe_contents = tokio::fs::read_to_string(&init_file)
            .await
            .context("failed to read contents of init file")?;

        maybe_contents.split(';')
    } else {
        DEFAULT_SQL.split(';')
    };

    println!("initializing databae");

    for statement in commands {
        let trimmed = statement.trim();

        if trimmed.is_empty() {
            continue;
        }

        client.execute(trimmed, &[])
            .await
            .with_context(|| format!("failed to run sql query\n{trimmed}"))?;
    }

    std::mem::drop(client);

    handle.await
        .context("failed to join db connection")?;

    Ok(())
}
