use axum::body::Body;
use axum::http::{Uri, StatusCode, HeaderName, HeaderValue};
use axum::http::response::Builder;
use axum::response::{IntoResponse, Response};

use crate::error::{self, Context};

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

pub fn spa_html(templates: &tera::Tera) -> Result<Response, error::Error> {
    let context = tera::Context::new();

    let page_index = templates.render("pages/spa", &context)
        .context("failed to render index page")?;

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .header("content-length", page_index.len())
        .body(page_index.into())
        .context("failed create response")
}

pub struct Html<T = String>{
    builder: Builder,
    body: T
}

impl<T> Html<T> {
    pub fn new(body: T) -> Self {
        let builder = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/html; charset=utf-8");

        Self {
            builder,
            body
        }
    }

    pub fn header<K, V>(self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<axum::http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<axum::http::Error>
    {
        Self {
            builder: self.builder.header(key, value),
            body: self.body
        }
    }
}

impl IntoResponse for Html<String> {
    fn into_response(self) -> Response<Body> {
        self.builder
            .header("content-length", self.body.len())
            .body(self.body.into())
            .unwrap()
    }
}

impl IntoResponse for Html<&str> {
    fn into_response(self) -> Response<Body> {
        let owned = self.body.to_owned();

        self.builder
            .header("content-length", owned.len())
            .body(owned.into())
            .unwrap()
    }
}
