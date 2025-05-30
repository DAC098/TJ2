use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct SecArg {
    #[command(subcommand)]
    cmd: SecCmd,
}

#[derive(Debug, Subcommand)]
enum SecCmd {
    /// handles creating private key data
    PkiCreate(PkiCreateArg),

    /// handles reading private key data
    PkiRead(PkiReadArg),
}

pub async fn handle(sec: SecArg) -> anyhow::Result<()> {
    match sec.cmd {
        SecCmd::PkiCreate(pki_args) => handle_pki_create(pki_args).await,
        SecCmd::PkiRead(pki_args) => handle_pki_read(pki_args).await,
    }
}

#[derive(Debug, Args)]
struct PkiCreateArg {
    /// overwrites a previously existing file at the desired location
    #[arg(long)]
    overwrite: bool,

    /// the output directory to send the public/private keys to
    output: PathBuf,
}

async fn handle_pki_create(pk: PkiCreateArg) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("failed to retrieve current working directory")?;

    let normalized = tj2_lib::path::normalize_from(&cwd, &pk.output);
    let metadata = tj2_lib::path::metadata(&normalized)
        .context("failed to lookup directory")?
        .context("output directory not found")?;

    if !metadata.is_dir() {
        anyhow::bail!("output path is not a directory");
    }

    let private_key_path = normalized.join(format!("private.key"));
    let private_key =
        tj2_lib::sec::pki::PrivateKey::generate().context("failed to generate private key")?;

    private_key
        .save(&private_key_path, pk.overwrite)
        .await
        .context("failed to save private key")?;

    Ok(())
}

#[derive(Debug, Args)]
struct PkiReadArg {
    /// the private key file to read
    input: PathBuf,
}

async fn handle_pki_read(pk: PkiReadArg) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("failed to retrieve current working directory")?;

    let normalized = tj2_lib::path::normalize_from(&cwd, &pk.input);

    let private_key = tj2_lib::sec::pki::PrivateKey::load(&normalized)
        .await
        .context("failed to load private key")?;

    let bytes = private_key.secret().to_bytes();
    let hex = tj2_lib::string::to_hex_str(&bytes);

    println!("created: {}", private_key.created());
    println!("bytes:   {hex}");

    Ok(())
}
