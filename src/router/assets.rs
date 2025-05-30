use std::fmt::Write;
use std::path::Path;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Method, StatusCode, Uri};
use axum::response::Response;
use tokio::fs::OpenOptions;
use tokio_util::io::ReaderStream;

use crate::error;
use crate::path;
use crate::state::SharedState;

fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from("asset not found"))
        .unwrap()
}

fn server_error() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from("server error"))
        .unwrap()
}

fn bad_request() -> Response<Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from("bad reqest"))
        .unwrap()
}

fn method_not_allowed() -> Response<Body> {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from("method not allowed"))
        .unwrap()
}

fn get_mime(path: &Path) -> String {
    let guess = mime_guess::MimeGuess::from_path(path);

    if guess.count() <= 1 {
        guess.first_or_octet_stream().to_string()
    } else {
        let mut iter = guess.iter();
        let mut rtn = iter.next().unwrap().to_string();

        for found in iter {
            write!(&mut rtn, ",{found}").unwrap();
        }

        rtn
    }
}

async fn send_file(path: &Path) -> Response<Body> {
    //tracing::debug!("attempting to send file: \"{}\"", path.display());

    let mime = get_mime(path);

    let metadata = match path::metadata(path) {
        Ok(maybe) => {
            if let Some(meta) = maybe {
                meta
            } else {
                return not_found();
            }
        }
        Err(err) => {
            error::log_prefix_error("error when retrieving metadata for asset file", &err);

            return server_error();
        }
    };

    if !metadata.is_file() {
        return bad_request();
    }

    let file = match OpenOptions::new().read(true).open(path).await {
        Ok(file) => file,
        Err(err) => {
            error::log_prefix_error("error when opening asset file", &err);

            return server_error();
        }
    };

    let stream = ReaderStream::new(file);
    let result = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.to_string())
        .header(header::CONTENT_LENGTH, metadata.len())
        .body(Body::from_stream(stream));

    match result {
        Ok(res) => res,
        Err(err) => {
            error::log_prefix_error("error when creating asset response", &err);

            server_error()
        }
    }
}

pub async fn handle(State(state): State<SharedState>, method: Method, uri: Uri) -> Response<Body> {
    if method != Method::GET {
        return method_not_allowed();
    }

    let uri_path = uri.path();

    //tracing::debug!("searching for: \"{uri_path}\"");

    if let Some(asset) = state.assets().get_file(uri_path) {
        send_file(asset).await
    } else if let Some((dir, stripped)) = state.assets().get_dir(uri_path) {
        let parts = stripped.split('/');
        let mut working = dir.to_path_buf();

        for part in parts {
            if part == ".." || part == "." {
                return bad_request();
            } else {
                working.push(part);
            }
        }

        send_file(&working).await
    } else {
        not_found()
    }
}
