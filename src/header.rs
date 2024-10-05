use std::iter::Iterator;
use std::str::FromStr;

use axum::http::HeaderMap;
use axum::http::header::ToStrError;

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
