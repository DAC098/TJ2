macro_rules! require_initiator {
    ($conn:expr, $headers:expr, $uri:expr) => {
        match crate::sec::authn::Initiator::from_headers($conn, $headers).await {
            Ok(value) => value,
            Err(err) => match err {
                crate::sec::authn::InitiatorError::HeaderStr(err) =>
                    return Err(crate::error::Error::context_source(
                        "failed to parse request headers",
                        err
                    )),
                crate::sec::authn::InitiatorError::Token(err) =>
                    return Err(crate::error::Error::context_source(
                        "failed to parse session token",
                        err
                    )),
                crate::sec::authn::InitiatorError::Db(err) => 
                    return Err(crate::error::Error::context_source(
                        "database error when retrieve session",
                        err
                    )),
                err => {
                    crate::error::log_prefix_error(
                        "failed to retrieve request initiator",
                        &err
                    );

                    return Ok(crate::router::responses::login_redirect($uri));
                }
            }
        }
    }
}

pub(crate) use require_initiator;

macro_rules! require_initiator_pg {
    ($conn:expr, $headers:expr, $uri:expr) => {
        match crate::sec::authn::Initiator::from_headers_pg($conn, $headers).await {
            Ok(value) => value,
            Err(err) => match err {
                crate::sec::authn::InitiatorError::HeaderStr(err) =>
                    return Err(crate::error::Error::context_source(
                        "failed to parse request headers",
                        err
                    )),
                crate::sec::authn::InitiatorError::Token(err) =>
                    return Err(crate::error::Error::context_source(
                        "failed to parse session token",
                        err
                    )),
                crate::sec::authn::InitiatorError::DbPg(err) =>
                    return Err(crate::error::Error::context_source(
                        "database error when retrieving session",
                        err
                    )),
                err => {
                    crate::error::log_prefix_error(
                        "failed to retrieve request initiator",
                        &err
                    );

                    return Ok(crate::router::responses::login_redirect($uri));
                }
            }
        }
    }
}

pub(crate) use require_initiator_pg;

macro_rules! res_if_html {
    ($templates:expr, $headers:expr) => {
        let Ok(is_html) = crate::header::is_accepting_html($headers) else {
            let body = "invalid characters in accept header";

            return Ok(axum::response::Response::builder()
                .status(axum::http::StatusCode::BAD_REQUEST)
                .header("content-type", "text/plain; charset=utf-8")
                .header("content-length", body.len())
                .body(axum::body::Body::from(body))
                .unwrap());
        };

        if is_html {
            return Ok(crate::router::body::SpaPage::new($templates)?
                .into_response())
        }
    }
}

pub(crate) use res_if_html;
