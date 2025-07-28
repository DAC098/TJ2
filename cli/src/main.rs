use anyhow::Context;
use clap::{Parser, Subcommand};

mod db;
mod sec;

/// a command line utility for managing / developing a TJ2 server
#[derive(Debug, Parser)]
struct AppCli {
    #[command(subcommand)]
    cmd: AppCmd,
}

#[derive(Debug, Subcommand)]
enum AppCmd {
    /// commands specific to security related features of the server
    Sec(sec::SecArg),

    /// commands specific to the database
    Db(db::DbArg),
}

fn main() -> anyhow::Result<()> {
    let args = AppCli::parse();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create tokio runtime")?
        .block_on(run(args))
}

async fn run(args: AppCli) -> anyhow::Result<()> {
    match args.cmd {
        AppCmd::Sec(sec) => sec::handle(sec).await,
        AppCmd::Db(db) => db::handle(db).await,
    }
}
