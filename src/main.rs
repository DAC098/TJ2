use axum::Router;
use axum::routing::get;
use clap::Parser;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use tokio::runtime::Builder;
use tracing_subscriber::{FmtSubscriber, EnvFilter};

mod error;
mod path;
mod config;

use error::{Error, Context};

fn main() {
    let args = config::CliArgs::parse();
    let filter = EnvFilter::from_default_env();

    if let Err(err) = FmtSubscriber::builder()
        .with_env_filter(filter)
        .try_init()
        .context("failed to initialize stdout logging") {
        error::print_error_stack(&err);

        std::process::exit(1);
    }

    let config = match config::Config::from_args(&args) {
        Ok(config) => config,
        Err(err) => {
            error::print_error_stack(&err);

            std::process::exit(1);
        }
    };

    if let Err(err) = setup(config) {
        error::print_error_stack(&err);

        std::process::exit(1);
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
    let router = Router::new()
        .route("/", get(retrieve_root))
        .with_state(());

    let mut all_futs = FuturesUnordered::new();

    for listener in config.settings.listeners {
        let local_router = router.clone();

        all_futs.push(tokio::spawn(async move {
            if let Err(err) = start_server(listener, local_router).await {
                error::print_error_stack(&err);
            }
        }));
    }

    while (all_futs.next().await).is_some() {
    }

    Ok(())
}

async fn start_server(listener: config::Listener, router: Router<()>) -> Result<(), error::Error> {
    let tcp_listener = {
        let err_msg = format!("failed binding to listener address {}", listener.addr);

        std::net::TcpListener::bind(listener.addr)
            .context(err_msg)?
    };

    match tcp_listener.local_addr() {
        Ok(addr) => tracing::info!("listening on: {addr}"),
        Err(err) => tracing::warn!("failed getting listener addr: {err}")
    }

    axum_server::from_tcp(tcp_listener)
        .serve(router.into_make_service())
        .await
        .context("error when running server")
}

async fn retrieve_root() -> &'static str {
    "root"
}
