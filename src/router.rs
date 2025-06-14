use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::error_handling::HandleErrorLayer;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Serialize;
use tower::ServiceBuilder;
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::error::{self, Context};
use crate::state;

mod assets;
mod layer;

pub mod body;
pub mod handles;
pub mod macros;

mod admin;
mod api;
mod journals;
mod login;
mod logout;
mod peers;
mod register;
mod settings;
mod verify;

pub async fn ping() -> (StatusCode, &'static str) {
    (StatusCode::OK, "pong")
}

#[derive(Debug, Serialize)]
pub struct RootJson {
    message: String,
}

async fn retrieve_root(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    let conn = state
        .db()
        .get()
        .await
        .context("failed to retrieve database connection")?;

    macros::require_initiator!(&conn, &headers, Some(uri));
    macros::res_if_html!(state.templates(), &headers);

    Ok(body::Json(RootJson {
        message: String::from("okay"),
    })
    .into_response())
}

async fn handle_error<E>(error: E) -> error::Error
where
    E: Into<error::Error>,
{
    let wrapper = error.into();

    error::log_prefix_error("uncaught error in middleware", &wrapper);

    wrapper
}

pub fn build(state: &state::SharedState) -> Router {
    Router::new()
        .route("/", get(retrieve_root))
        .route("/ping", get(ping))
        .route("/login", get(login::get).post(login::post))
        .route("/verify", get(verify::get).post(verify::post))
        .route("/logout", post(logout::post))
        .route("/register", get(handles::send_html).post(register::post))
        .route("/peers", get(peers::get))
        .nest("/journals", journals::build(state))
        .nest("/settings", settings::build(state))
        .nest("/admin", admin::build(state))
        .nest("/api", api::build(state))
        .fallback(assets::handle)
        .layer(
            ServiceBuilder::new()
                .layer(layer::RIDLayer::new())
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(make_span_with)
                        .on_request(on_request)
                        .on_response(on_response)
                        .on_failure(on_failure),
                )
                .layer(HandleErrorLayer::new(handle_error))
                .layer(layer::TimeoutLayer::new(Duration::new(90, 0))),
        )
        .with_state(state.clone())
}

fn make_span_with(request: &Request<Body>) -> Span {
    let req_id = layer::RequestId::from_request(request).expect("missing request id");
    let socket = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .expect("missing connect info");

    tracing::info_span!(
        "REQ",
        ip = %socket.0,
        id = req_id.id(),
        ver = ?request.version(),
        mth = %request.method(),
        pth = %request.uri().path(),
        qry = %request.uri().query().unwrap_or(""),
        sts = tracing::field::Empty
    )
}

fn on_request(_request: &Request<Body>, _span: &Span) {}

fn on_response(response: &Response<Body>, latency: Duration, span: &Span) {
    span.record("sts", tracing::field::display(response.status()));

    tracing::info!("{:#?}", latency)
}

fn on_failure(error: ServerErrorsFailureClass, latency: Duration, _span: &Span) {
    tracing::error!("{error} {:#?}", latency)
}
