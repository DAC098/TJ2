use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::error_handling::HandleErrorLayer;
use axum::extract::ConnectInfo;
use axum::http::Request;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use tower::ServiceBuilder;
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::state;
use crate::net;
use crate::net::response::send_html;

mod assets;
mod layer;

pub mod handles;

async fn handle_error<E>(error: E) -> net::Error
where
    E: Into<net::Error>,
{
    error.into()
}

pub fn build(state: &state::SharedState) -> Router {
    Router::new()
        .route("/", get(handles::retrieve_root))
        .route("/ping", get(handles::ping))
        .route("/login", get(handles::login::get).post(handles::login::post))
        .route("/verify", get(handles::verify::get).post(handles::verify::post))
        .route("/logout", post(handles::logout::post))
        .route("/register", get(send_html).post(handles::register::post))
        .route("/me", get(handles::me::retrieve_me))
        .route("/peers", get(handles::peers::get))
        .nest("/journals", handles::journals::build(state))
        .nest("/settings", handles::settings::build(state))
        .nest("/admin", handles::admin::build(state))
        .nest("/api", handles::api::build(state))
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
