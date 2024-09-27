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
