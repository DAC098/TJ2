use std::iter::Iterator;
use std::str::FromStr;

use axum::body::Body;
use axum::http::{Uri, StatusCode, HeaderMap, HeaderValue};
use axum::http::header::{ToStrError, InvalidHeaderValue};
use axum::response::{Response, ResponseParts, IntoResponse, IntoResponseParts};

use crate::error::{self, Context};

pub struct AcceptIter<'a> {
    iter: std::str::Split<'a, &'static str>
}

impl<'a> AcceptIter<'a> {
    pub fn from_headers(headers: &'a HeaderMap) -> Result<Option<Self>, ToStrError> {
        if let Some(accept) = headers.get("accept") {
            let accept_str = accept.to_str()?;

            Ok(Some(Self {
                iter: accept_str.split(",")
            }))
        } else {
            Ok(None)
        }
    }
}

impl<'a> Iterator for AcceptIter<'a> {
    type Item = mime::Mime;

    fn next(&mut self) -> Option<Self::Item> {
        for step in self.iter.by_ref() {
            let trimmed = step.trim();

            let Ok(mime) = mime::Mime::from_str(trimmed) else {
                continue;
            };

            return Some(mime);
        }

        None
    }
}

pub fn is_accepting_html(headers: &HeaderMap) -> Result<bool, ToStrError> {
    if let Some(iter) = AcceptIter::from_headers(headers)? {
        for mime in iter {
            if mime.type_() == "text" && mime.subtype() == "html" {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub struct Location(String);

impl Location {
    pub fn to(location: String) -> Self {
        Self(location)
    }

    pub fn login<U>(maybe_prev: Option<U>) -> Self 
    where
        Uri: TryFrom<U>
    {
        if let Some(prev) = maybe_prev {
            let Ok(uri): Result<Uri, _> = prev.try_into() else {
                panic!("invalid uri given to login redirect");
            };

            if let Some(path_query) = uri.path_and_query() {
                let encoded = urlencoding::encode(path_query.as_str());

                Self(format!("/login?prev={encoded}"))
            } else {
                Self(String::from("/login"))
            }
        } else {
            Self(String::from("/login"))
        }
    }

    pub fn into_header_value(self) -> Result<HeaderValue, InvalidHeaderValue> {
        HeaderValue::from_str(&self.0)
    }
}

impl TryFrom<Location> for HeaderValue {
    type Error = InvalidHeaderValue;

    fn try_from(value: Location) -> Result<Self, Self::Error> {
        value.into_header_value()
    }
}

impl IntoResponseParts for Location {
    type Error = error::Error;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let value = self.into_header_value()
            .context("failed to change Redirect into HeaderValue")?;

        res.headers_mut().insert("location", value);

        Ok(res)
    }
}

impl IntoResponse for Location {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::FOUND)
            .header("location", self)
            .body(Body::empty())
            .unwrap()
    }
}
