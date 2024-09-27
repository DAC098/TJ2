use axum::body::Body;
use axum::http::{Uri, StatusCode};
use axum::response::Response;

pub fn login_redirect(maybe_prev: Option<Uri>) -> Response {
    let location = if let Some(prev) = maybe_prev {
        if let Some(path_query) = prev.path_and_query() {
            let encoded = urlencoding::encode(path_query.as_str());

            format!("/login?prev={encoded}")
        } else {
            "/login".to_owned()
        }
    } else {
        "/login".to_owned()
    };

    Response::builder()
        .status(StatusCode::FOUND)
        .header("location", location)
        .body(Body::empty())
        .unwrap()
}
