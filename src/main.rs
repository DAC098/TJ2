use std::net::{SocketAddr, TcpListener};

use axum::Router;
use clap::Parser;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::runtime::Builder;

mod config;
mod db;
mod error;
mod fs;
mod jobs;
mod logging;
mod net;
mod path;
mod sec;
mod state;
mod stream;
mod sync;
mod templates;

mod journal;
mod user;

mod api;
mod router;

use error::{Context, Error};

fn main() {
    let args = config::CliArgs::parse();

    let (config, guard) = tracing::subscriber::with_default(logging::stdio_subscriber(), || {
        let config =
            error::trace_and_exit!(config::Config::from_args(&args), "failed to create config");
        let guard = error::trace_and_exit!(logging::init(&config), "failed to initialize logging");

        (config, guard)
    });

    let successful = error::trace_pass!(entry(args, config)).is_ok();

    drop(guard);

    std::process::exit(if successful { 1 } else { 0 });
}

fn entry(args: config::CliArgs, config: config::Config) -> Result<(), Error> {
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

    rt.block_on(run(args, config))
}

/// initializes state, router configuration, database setup, and then starts
async fn run(args: config::CliArgs, config: config::Config) -> Result<(), Error> {
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
