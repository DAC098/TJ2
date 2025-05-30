macro_rules! require_initiator {
    ($conn:expr, $headers:expr, $uri:expr) => {
        match crate::sec::authn::Initiator::from_headers($conn, $headers).await {
            Ok(value) => value,
            Err(err) => {
                match err {
                    crate::sec::authn::InitiatorError::DbPg(err) => {
                        return Err(crate::error::Error::context_source(
                            "database error when retrieving session",
                            err,
                        ))
                    }
                    err => {
                        crate::error::log_prefix_error("failed to retrieve request initiator", &err)
                    }
                }

                return Ok(crate::header::Location::login($uri).into_response());
            }
        }
    };
}

pub(crate) use require_initiator;

macro_rules! res_if_html {
    ($templates:expr, $headers:expr) => {
        let Ok(is_html) = crate::header::is_accepting_html($headers) else {
            return Ok((
                axum::http::StatusCode::BAD_REQUEST,
                "invalid characters in accept header",
            )
                .into_response());
        };

        if is_html {
            return Ok(crate::router::body::SpaPage::new($templates)?.into_response());
        }
    };
}

pub(crate) use res_if_html;
