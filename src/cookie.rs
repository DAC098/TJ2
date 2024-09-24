use std::time::Duration;

use axum::body::Body;
use axum::http::{StatusCode, HeaderValue, header::InvalidHeaderValue};
use axum::response::{
    Response,
    ResponseParts,
    IntoResponse,
    IntoResponseParts
};
use chrono::{DateTime, Utc};

use crate::error::{self, Context};

pub enum SameSite {
    Strict,
    Lax,
    None
}

impl SameSite {
    pub fn as_str(&self) -> &str {
        match self {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
            SameSite::None => "None"
        }
    }
}

pub struct SetCookie {
    pub key: String,
    pub value: String,

    pub expires: Option<DateTime<Utc>>,
    pub max_age: Option<Duration>,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<SameSite>
}

impl SetCookie {
    pub fn new<K,V>(key: K, value: V) -> SetCookie
    where
        K: Into<String>,
        V: Into<String>
    {
        SetCookie {
            key: key.into(),
            value: value.into(),
            expires: None,
            max_age: None,
            domain: None,
            path: None,
            secure: false,
            http_only: false,
            same_site: None
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn expires(&self) -> Option<&DateTime<Utc>> {
        self.expires.as_ref()
    }

    pub fn max_age(&self) -> Option<&Duration> {
        self.max_age.as_ref()
    }

    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub fn secure(&self) -> &bool {
        &self.secure
    }

    pub fn http_only(&self) -> &bool {
        &self.http_only
    }

    pub fn same_site(&self) -> Option<&SameSite> {
        self.same_site.as_ref()
    }

    pub fn set_expires(&mut self, expires: DateTime<Utc>) -> &mut Self {
        self.expires = Some(expires);
        self
    }

    pub fn with_expires(mut self, expires: DateTime<Utc>) -> Self {
        self.expires = Some(expires);
        self
    }

    pub fn set_max_age(&mut self, max_age: Duration) -> &mut Self {
        self.max_age = Some(max_age);
        self
    }

    pub fn with_max_age(mut self, max_age: Duration) -> Self {
        self.max_age = Some(max_age);
        self
    }

    pub fn set_domain<D>(&mut self, domain: D) -> &mut Self
    where
        D: Into<String>
    {
        self.domain = Some(domain.into());
        self
    }

    pub fn with_domain<D>(mut self, domain: D) -> Self
    where
        D: Into<String>
    {
        self.domain = Some(domain.into());
        self
    }

    pub fn set_path<P>(&mut self, path: P) -> &mut Self
    where
        P: Into<String>
    {
        self.path = Some(path.into());
        self
    }

    pub fn with_path<P>(mut self, path: P) -> Self
    where
        P: Into<String>
    {
        self.path = Some(path.into());
        self
    }

    pub fn set_secure(&mut self, secure: bool) -> &mut Self {
        self.secure = secure;
        self
    }

    pub fn with_secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    pub fn set_http_only(&mut self, http_only: bool) -> &mut Self {
        self.http_only = http_only;
        self
    }

    pub fn with_http_only(mut self, http_only: bool) -> Self {
        self.http_only = http_only;
        self
    }

    pub fn set_same_site(&mut self, same_site: SameSite) -> &mut Self {
        self.same_site = Some(same_site);
        self
    }

    pub fn with_same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = Some(same_site);
        self
    }

    pub fn into_header_value(self) -> Result<HeaderValue, InvalidHeaderValue> {
        let mut rtn = format!("{}={}", self.key, self.value);

        if let Some(expire) = self.expires {
            let date = expire.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
            rtn.push_str("; Expires=");
            rtn.push_str(date.as_str());
        }

        if let Some(duration) = self.max_age {
            let seconds = duration.as_secs().to_string();
            rtn.push_str("; Max-Age=");
            rtn.push_str(seconds.as_str());
        }

        if let Some(domain) = self.domain {
            rtn.push_str("; Domain=");
            rtn.push_str(domain.as_str());
        }

        if let Some(path) = self.path {
            rtn.push_str("; Path=");
            rtn.push_str(path.as_str());
        }

        if self.secure {
            rtn.push_str("; Secure");
        }

        if self.http_only {
            rtn.push_str("; HttpOnly");
        }

        if let Some(same_site) = self.same_site {
            rtn.push_str("; SameSite=");
            rtn.push_str(same_site.as_str());
        }

        HeaderValue::from_str(&rtn)
    }
}

impl TryFrom<SetCookie> for HeaderValue {
    type Error = InvalidHeaderValue;

    fn try_from(value: SetCookie) -> Result<Self, Self::Error> {
        value.into_header_value()
    }
}

impl IntoResponseParts for SetCookie {
    type Error = error::Error;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let value = self.into_header_value()
            .context("failed to to change SetCookie into HeaderValue")?;

        res.headers_mut().insert("set-cookie", value);

        Ok(res)
    }
}

impl IntoResponse for SetCookie {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .header("set-cookie", self)
            .body(Body::empty())
            .unwrap()
    }
}
