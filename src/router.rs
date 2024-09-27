use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::error_handling::HandleErrorLayer;
use axum::http::{Uri, Request, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tower_http::classify::ServerErrorsFailureClass;
use tera::Context as TeraContext;
use tracing::Span;

use crate::state;
use crate::error::{self, Context};

mod layer;
mod assets;

pub mod responses;
pub mod macros;

mod auth;

async fn ping() -> (StatusCode, &'static str) {
    (StatusCode::OK, "pong")
}

async fn retrieve_root(
    state: state::SharedState,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    let mut conn = state.db()
        .acquire()
        .await
        .context("failed to retrieve database connection")?;

    macros::require_initiator!(&mut conn, &headers, Some(uri));

    let mut context = TeraContext::new();
    context.insert("title", &"Root Page");

    let page_index = state.templates()
        .render("pages/index", &context)
        .context("failed to render index page")?;

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .header("content-length", page_index.len())
        .body(page_index.into())
        .context("failed create response")
}

async fn handle_error<E>(error: E) -> error::Error
where
    E: Into<error::Error>
{
    let wrapper = error.into();

    error::log_prefix_error("uncaught error in middleware", &wrapper);

    wrapper
}

pub fn build(state: &state::SharedState) -> Router {
    Router::new()
        .route("/", get(retrieve_root))
        .route("/ping", get(ping))
        .route("/login", get(auth::login)
            .post(auth::request_login))
        .fallback(assets::handle)
        .layer(ServiceBuilder::new()
            .layer(layer::RIDLayer::new())
            .layer(TraceLayer::new_for_http()
                .make_span_with(make_span_with)
                .on_request(on_request)
                .on_response(on_response)
                .on_failure(on_failure))
            .layer(HandleErrorLayer::new(handle_error))
            .layer(layer::TimeoutLayer::new(Duration::new(90, 0))))
        .with_state(state.clone())
}

fn make_span_with(request: &Request<Body>) -> Span {
    let req_id = layer::RequestId::from_request(request).expect("missing request id");

    tracing::info_span!(
        "REQ",
        i = req_id.id(),
        v = ?request.version(),
        m = %request.method(),
        u = %request.uri(),
        s = tracing::field::Empty
    )
}

fn on_request(_request: &Request<Body>, _span: &Span) {}

fn on_response(response: &Response<Body>, latency: Duration, span: &Span) {
    span.record("s", tracing::field::display(response.status()));

    tracing::info!("{:#?}", latency)
}

fn on_failure(error: ServerErrorsFailureClass, latency: Duration, _span: &Span) {
    tracing::error!("{error} {:#?}", latency)
}
