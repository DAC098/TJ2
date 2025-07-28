use anyhow::Context;
use clap::{Args, Subcommand};
use refinery::{Migration, Runner};
use tokio_postgres::Client;

mod embedded {
    use refinery::embed_migrations;

    // from the root of the package not the file
    embed_migrations!("../db/migrates");
}

#[derive(Debug, Args)]
pub struct MigrateArgs {
    #[command(subcommand)]
    cmd: MigrateCmd,
}

#[derive(Debug, Subcommand)]
enum MigrateCmd {
    /// runs migrates for the database
    Run,

    /// list applied migrations
    Applied(AppliedArgs),
}

pub async fn handle(client: &mut Client, args: MigrateArgs) -> anyhow::Result<()> {
    let runner = embedded::migrations::runner();

    match args.cmd {
        MigrateCmd::Run => handle_run(client, runner).await,
        MigrateCmd::Applied(args) => handle_applied(client, runner, args).await,
    }
}

async fn handle_run(client: &mut Client, runner: Runner) -> anyhow::Result<()> {
    let result = runner
        .run_async(client)
        .await
        .context("failed to run migrates")?;

    println!("migrates applied");

    for migrate in result.applied_migrations() {
        print_list_migrate(&migrate);
    }

    Ok(())
}

#[derive(Debug, Args)]
pub struct AppliedArgs {
    /// the last applied migration
    #[arg(long)]
    last: bool,
}

async fn handle_applied(
    client: &mut Client,
    runner: Runner,
    AppliedArgs { last }: AppliedArgs,
) -> anyhow::Result<()> {
    if last {
        if let Some(last) = runner
            .get_last_applied_migration_async(client)
            .await
            .context("failed to retrieve last applied migration")?
        {
            println!("last applied migration");

            print_migrate(&last);
        } else {
            println!("no previously applied migration");
        }
    } else {
        let applied = runner
            .get_applied_migrations_async(client)
            .await
            .context("failed to retrieve applied migrations")?;

        println!("applied migrations");

        for migrate in applied {
            print_list_migrate(&migrate);
        }
    }

    Ok(())
}

fn print_migrate(migration: &Migration) {
    match migration.applied_on() {
        Some(date) => println!(
            "{}{}) {}\nchecksum: {}\ndate: {date}",
            migration.prefix(),
            migration.version(),
            migration.name(),
            migration.checksum()
        ),
        None => println!(
            "{}{}) {}\nchecksum: {}",
            migration.prefix(),
            migration.version(),
            migration.name(),
            migration.checksum()
        ),
    }
}

fn print_list_migrate(migration: &Migration) {
    match migration.applied_on() {
        Some(date) => println!(
            "  - {}{}) {}\n    checksum: {}\n    date: {date}",
            migration.prefix(),
            migration.version(),
            migration.name(),
            migration.checksum()
        ),
        None => println!(
            "  - {}{}) {}\n    checksum: {}",
            migration.prefix(),
            migration.version(),
            migration.name(),
            migration.checksum()
        ),
    }
}
