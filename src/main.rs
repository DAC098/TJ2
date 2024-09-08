use std::path::PathBuf;

use tokio::runtime::Builder;

mod error;

use error::{Error, Context};

#[derive(Debug, clap::Parser)]
struct CliArgs {
    config_path: PathBuf
}

fn main() {
    if let Err(err) = setup() {
        error::print_error_stack(&err);

        std::process::exit(err.code);
    } else {
        std::process::exit(0);
    }
}

fn setup() -> Result<(), Error> {
    let rt = Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .max_blocking_threads(4)
        .build()
        .context("failed to create tokio runtime")?;

    Err(Error::code(1, "test error"))
}

//fn init_server() -> Result<
