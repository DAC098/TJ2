use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct SecArg {
    #[command(subcommand)]
    cmd: SecCmd
}

#[derive(Debug, Subcommand)]
enum SecCmd {
    /// handles server private key data
    Pk(PkArg)
}

#[derive(Debug, Args)]
struct PkArg {
    /// overwrites a previously existing file at the desired location
    #[arg(long)]
    overwrite: bool,

    /// the output directory to send the public/private keys to
    output: PathBuf
}

pub async fn handle(sec: SecArg) -> anyhow::Result<()> {
    match sec.cmd {
        SecCmd::Pk(pk_args) => handle_pk(pk_args).await
    }
}

async fn handle_pk(pk: PkArg) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()
        .context("failed to retrieve current working directory")?;

    let normalized = tj2_lib::path::normalize_from(&cwd, &pk.output);
    let metadata = tj2_lib::path::metadata(&normalized)
        .context("failed to lookup directory")?
        .context("output directory not found")?;

    if !metadata.is_dir() {
        anyhow::bail!("output path is not a directory");
    }

    let private_key = tj2_lib::sec::pki::gen_private_key()
        .context("failed to generate private key")?;
    let private_key_path = normalized.join(format!("private.key"));

    tj2_lib::sec::pki::save_private_key(&private_key_path, &private_key, pk.overwrite)
        .await
        .context("failed to save private key")?;

    Ok(())
}
