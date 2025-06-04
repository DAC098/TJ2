use std::net::{SocketAddr, TcpListener};

use axum::Router;
use clap::Parser;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::runtime::Builder;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

mod config;
mod db;
mod error;
mod fs;
mod jobs;
mod net;
mod path;
mod sec;
mod serde;
mod state;
mod sync;
mod templates;

mod journal;
mod user;

mod api;
mod router;

use error::{Context, Error};

fn main() {
    let args = config::CliArgs::parse();
    let mut filter = EnvFilter::from_default_env();

    if let Some(verbosity) = &args.verbosity {
        let log_str = match verbosity {
            config::Verbosity::Error => "TJ2=error",
            config::Verbosity::Warn => "TJ2=warn",
            config::Verbosity::Info => "TJ2=info",
            config::Verbosity::Debug => "TJ2=debug",
            config::Verbosity::Trace => "TJ2=trace",
        };

        filter = filter.add_directive(log_str.parse().unwrap());
    }

    if let Err(err) = FmtSubscriber::builder()
        .with_env_filter(filter)
        .try_init()
        .context("failed to initialize stdout logging")
    {
        error::log_error(&err);

        std::process::exit(1);
    }

    let config = match config::Config::from_args(&args) {
        Ok(config) => config,
        Err(err) => {
            error::log_error(&err);

            std::process::exit(1);
        }
    };

    if let Err(err) = setup(args, config) {
        error::log_error(&err);

        std::process::exit(1);
    } else {
        std::process::exit(0);
    }
}

/// configures the tokio runtime and starts the init process for the server
fn setup(args: config::CliArgs, config: config::Config) -> Result<(), Error> {
    let mut builder = if config.settings.thread_pool == 1 {
        Builder::new_current_thread()
    } else {
        Builder::new_multi_thread()
    };

    let rt = builder
        .enable_io()
        .enable_time()
        .max_blocking_threads(config.settings.blocking_pool)
        .build()
        .context("failed to create tokio runtime")?;

    rt.block_on(init(args, config))
}

/// initializes the server with the shared state, router configuration, and
/// database setup
async fn init(args: config::CliArgs, config: config::Config) -> Result<(), Error> {
    let state = state::SharedState::new(&config)
        .await
        .context("failed to create SharedState")?;

    db::check_database(&state).await?;

    if args.gen_test_data {
        db::gen_test_data(&state).await?;
    }

    let router = router::build(&state);

    let mut server_handles = Vec::with_capacity(config.settings.listeners.len());
    let mut all_futs = FuturesUnordered::new();

    for listener in config.settings.listeners {
        let handle = axum_server::Handle::new();
        let local_router = router.clone();
        let local_handle = handle.clone();

        server_handles.push(handle);
        all_futs.push(tokio::spawn(start_server(
            listener,
            local_router,
            local_handle,
        )));
    }

    all_futs.push(tokio::spawn(handle_signal(server_handles)));

    while (all_futs.next().await).is_some() {}

    tracing::info!("closing database connections");

    state.db().close();

    Ok(())
}

/// creates a TCP lister socket with the given socket address
fn create_listener(addr: &SocketAddr) -> Result<TcpListener, error::Error> {
    let listener = std::net::TcpListener::bind(addr)
        .context(format!("failed binding to listener address {addr}"))?;

    if addr.port() == 0 {
        let local_addr = listener
            .local_addr()
            .expect("expected to retrieve a valid ipv4/v6 address for the listener socket");

        tracing::info!("listening on: {local_addr}");
    } else {
        tracing::info!("listening on: {addr}");
    }

    Ok(listener)
}

/// entry point for a tokio task to start the server
async fn start_server(listener: config::Listener, router: Router, handle: axum_server::Handle) {
    if let Err(err) = create_server(listener, router, handle).await {
        error::log_error(&err);
    }
}

/// creates an http server
///
/// if the listener is specified to be a tls server it will be ignored
#[cfg(not(feature = "rustls"))]
async fn create_server(
    listener: config::Listener,
    router: Router,
    handle: axum_server::Handle,
) -> Result<(), error::Error> {
    let listener = create_listener(&listener.addr)?;

    axum_server::from_tcp(listener)
        .handle(handle)
        .serve(router.into_make_service())
        .await
        .context("error when running server")
}

/// creates an http server
///
/// if the listener is specified to be a tls server it will attempt to create
/// the listener with the provided tls options.
#[cfg(feature = "rustls")]
async fn create_server(
    listener: config::Listener,
    router: Router,
    handle: axum_server::Handle,
) -> Result<(), error::Error> {
    use axum_server::tls_rustls::RustlsConfig;

    if let Some(tls) = listener.tls {
        let tls_config = RustlsConfig::from_pem_file(tls.cert, tls.key)
            .await
            .context(format!(
                "failed to load pem files for listener {}",
                listener.addr
            ))?;

        let listener = create_listener(&listener.addr)?;

        axum_server::from_tcp_rustls(listener, tls_config)
            .handle(handle)
            .serve(router.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .context("error when running server")
    } else {
        let listener = create_listener(&listener.addr)?;

        axum_server::from_tcp(listener)
            .handle(handle)
            .serve(router.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .context("error when running server")
    }
}

/// a signal handle that will shutdown all known listening servers
async fn handle_signal(handles: Vec<axum_server::Handle>) {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::error!("error when listening for ctrl-c. {err}");
    } else {
        tracing::info!("shuting down server listeners");

        for handle in handles {
            handle.shutdown();
        }
    }
}
