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
mod db;
mod state;

use error::{Error, Context};

fn main() {
    let args = config::CliArgs::parse();
    let mut filter = EnvFilter::from_default_env();

    if let Some(verbosity) = &args.verbosity {
        filter = match verbosity {
            config::Verbosity::Error => filter.add_directive("TJ2=error".parse().unwrap()),
            config::Verbosity::Warn => filter.add_directive("TJ2=warn".parse().unwrap()),
            config::Verbosity::Info => filter.add_directive("TJ2=info".parse().unwrap()),
            config::Verbosity::Debug => filter.add_directive("TJ2=debug".parse().unwrap()),
            config::Verbosity::Trace => filter.add_directive("TJ2=trace".parse().unwrap()),
        }
    }

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
    let state = state::SharedState::new(&config)
        .await
        .context("failed to create SharedState")?;

    let router = Router::new()
        .route("/", get(retrieve_root))
        .with_state(state.clone());

    let mut server_handles = Vec::with_capacity(config.settings.listeners.len());
    let mut all_futs = FuturesUnordered::new();

    for listener in config.settings.listeners {
        let handle = axum_server::Handle::new();
        let local_router = router.clone();
        let local_handle = handle.clone();

        server_handles.push(handle);
        all_futs.push(tokio::spawn(async move {
            if let Err(err) = start_server(listener, local_router, local_handle).await {
                error::print_error_stack(&err);
            }
        }));
    }

    all_futs.push(tokio::spawn(async move {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!("error when listening for ctrl-c. {err}");
        } else {
            tracing::info!("shuting down server listeners");

            for handle in server_handles {
                handle.shutdown();
            }
        }
    }));

    while (all_futs.next().await).is_some() {}

    Ok(())
}

async fn start_server(
    listener: config::Listener,
    router: Router,
    handle: axum_server::Handle
) -> Result<(), error::Error> {
    let tcp_listener = {
        let err_msg = format!("failed binding to listener address {}", listener.addr);

        std::net::TcpListener::bind(listener.addr)
            .context(err_msg)?
    };

    {
        // we should always get a valid addr because we will only be using v4/v6
        // addresses for the tcp listener
        let addr = tcp_listener.local_addr()
            .expect("expected to retrieve a valid ipv4/v6 address for the listener socket");

        tracing::info!("listening on: {addr}");
    }

    axum_server::from_tcp(tcp_listener)
        .handle(handle)
        .serve(router.into_make_service())
        .await
        .context("error when running server")
}

async fn retrieve_root() -> &'static str {
    "root"
}
