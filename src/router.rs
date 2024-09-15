use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::error_handling::HandleErrorLayer;
use axum::http::{Request, Response, StatusCode};
use axum::routing::get;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tower_http::classify::ServerErrorsFailureClass;
use tracing::Span;

use crate::state;
use crate::error;

mod layer;

async fn ping() -> (StatusCode, &'static str) {
    (StatusCode::OK, "pong")
}

async fn handle_error<E>(error: E) -> error::Error
where
    E: Into<error::Error>
{
    let wrapper = error.into();

    error::print_error_stack(&wrapper);

    wrapper
}

pub fn build(state: &state::SharedState) -> Router {
    Router::new()
        .route("/ping", get(ping))
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
    span.record("s", &tracing::field::display(response.status()));

    tracing::info!("{:#?}", latency)
}

fn on_failure(error: ServerErrorsFailureClass, latency: Duration, _span: &Span) {
    tracing::error!("{error} {:#?}", latency)
}
