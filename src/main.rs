use clap::Parser;
use tokio::runtime::Builder;

mod error;
mod path;
mod config;

use error::{Error, Context};

fn main() {
    let args = config::CliArgs::parse();
    let config = match config::Config::from_args(&args) {
        Ok(config) => config,
        Err(err) => {
            error::print_error_stack(&err);

            std::process::exit(err.code);
        }
    };

    if let Err(err) = setup(config) {
        error::print_error_stack(&err);

        std::process::exit(err.code);
    } else {
        std::process::exit(0);
    }
}

fn setup(config: config::Config) -> Result<(), Error> {
    let mut builder = if config.settings.thread_pool == 1 {
        Builder::new_current_thread()
    } else {
        Builder::new_multi_thread()
    };

    let rt = builder.enable_io()
        .enable_time()
        .max_blocking_threads(config.settings.blocking_pool)
        .build()
        .context("failed to create tokio runtime")?;

    rt.block_on(init(config))
}

async fn init(config: config::Config) -> Result<(), Error> {
    Ok(())
}
