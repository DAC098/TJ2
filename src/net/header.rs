use std::iter::Iterator;
use std::str::FromStr;

use axum::body::Body;
use axum::http::header::{InvalidHeaderValue, ToStrError};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, IntoResponseParts, Response, ResponseParts};

use crate::error::{self, Context};

/// an iterator over all values found in the "accept" header
pub struct AcceptIter<'a> {
    iter: std::str::Split<'a, &'static str>,
}

impl<'a> AcceptIter<'a> {
    /// attempts to retrieve all values in the "accept" header if present
    pub fn from_headers(headers: &'a HeaderMap) -> Result<Option<Self>, ToStrError> {
        if let Some(accept) = headers.get("accept") {
            let accept_str = accept.to_str()?;

            Ok(Some(Self {
                iter: accept_str.split(","),
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

/// checks to see if the header map contains the "accept" header nad is looking
/// for "text/html"
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

/// a helper struct for creating the "location" header for redirects
pub struct Location(String);

impl Location {
    /// creates the Location for a given value
    pub fn to<T>(location: T) -> Self
    where
        T: Into<String>,
    {
        Self(location.into())
    }

    /// creates a Location to the login page and will also include the given
    /// url as a prev query param if provided.
    ///
    /// if the url does not contain a path and or query value then only the
    /// login path will be specified.
    ///
    /// the function will panic if the provided value cannot be parsed to a
    /// valid [`Uri`]
    pub fn login<U>(maybe_prev: Option<U>) -> Self
    where
        Uri: TryFrom<U>,
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

    /// attempts to convert the struct into a [`HeaderValue`]
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
        let value = self
            .into_header_value()
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
