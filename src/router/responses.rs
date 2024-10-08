use axum::body::Body;
use axum::http::{Uri, StatusCode};
use axum::response::Response;

pub fn login_redirect<U>(maybe_prev: Option<U>) -> Response
where
    Uri: TryFrom<U>
{
    let location = if let Some(prev) = maybe_prev {
        let Ok(uri): Result<Uri, _> = prev.try_into() else {
            panic!("invalid uri given to login redirect");
        };

        if let Some(path_query) = uri.path_and_query() {
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
