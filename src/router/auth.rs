use argon2::{Argon2, PasswordVerifier};
use argon2::password_hash::PasswordHash;
use axum::Form;
use axum::extract::Query;
use axum::http::{StatusCode, HeaderMap};
use axum::body::Body;
use axum::response::Response;
use tera::Context as TeraContext;
use serde::Deserialize;
use sqlx::Row;

use crate::state;
use crate::error::{self, Context};
use crate::sec::authn::{Session, Initiator, InitiatorError};
use crate::sec::authn::session::SessionOptions;

#[derive(Debug, Default)]
struct LoginPageOptions {
    user_not_found: bool,
    invalid_password: bool,
    clear_session_id: bool,
    prev: Option<String>,
}

fn respond_login_page(
    state: &state::SharedState,
    options: LoginPageOptions,
) -> Result<Response, error::Error> {
    let mut login_context = TeraContext::new();
    login_context.insert("user_not_found", &options.user_not_found);
    login_context.insert("invalid_password", &options.invalid_password);

    let post_url = if let Some(prev) = options.prev {
        let encoded = urlencoding::encode(&prev);

        format!("/login?prev={encoded}")
    } else {
        "/login".to_owned()
    };

    login_context.insert("post_url", &post_url);

    let login_render = state.templates()
        .render("pages/login", &login_context)
        .context("failed to render the login page")?;

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .header("content-length", login_render.len());

    if options.clear_session_id {
        builder = builder.header("set-cookie", Session::clear_cookie());
    }

    builder.body(login_render.into())
        .context("failed to create login page response")
}

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    prev: Option<String>
}

impl LoginQuery {
    fn get_prev(&self) -> Option<String> {
        if let Some(prev) = &self.prev {
            match urlencoding::decode(prev) {
                Ok(cow) => Some(cow.into_owned()),
                Err(err) => {
                    tracing::warn!("failed to decode login prev query: {err}");

                    None
                }
            }
        } else {
            None
        }
    }
}

pub async fn login(
    state: state::SharedState,
    Query(query): Query<LoginQuery>,
    headers: HeaderMap,
) -> Result<Response, error::Error> {
    let mut conn = state.db()
        .acquire()
        .await
        .context("failed to retrieve database connection")?;

    let result = Initiator::from_headers(&mut conn, &headers)
        .await;

    match result {
        Ok(_) => {
            tracing::debug!("session for initiator is valid");

            let location = query.get_prev()
                .unwrap_or("/".to_owned());

            Response::builder()
                .status(StatusCode::FOUND)
                .header("location", location)
                .body(Body::empty())
                .context("failed to create redirect response")
        }
        Err(err) => match err {
            InitiatorError::SessionIdNotFound => {
                tracing::debug!("session id not found");

                let options = LoginPageOptions {
                    prev: query.get_prev(),
                    ..Default::default()
                };

                respond_login_page(&state, options)
            }
            InitiatorError::SessionNotFound |
            InitiatorError::UserNotFound(_) |
            InitiatorError::Unauthenticated(_) |
            InitiatorError::Unverified(_) |
            InitiatorError::SessionExpired(_) => {
                tracing::debug!("problem with session");

                let options = LoginPageOptions {
                    prev: query.get_prev(),
                    clear_session_id: true,
                    ..Default::default()
                };

                respond_login_page(&state, options)
            }
            InitiatorError::HeaderStr(err) => {
                Err(error::Error::context_source(
                    "error when parsing cookie headers",
                    err
                ))
            }
            InitiatorError::Token(err) => {
                Err(error::Error::context_source(
                    "invalid session token",
                    err
                ))
            }
            InitiatorError::Db(err) => {
                Err(error::Error::context_source(
                    "database error when retrieving session",
                    err
                ))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

pub async fn request_login(
    state: state::SharedState,
    Query(query): Query<LoginQuery>,
    Form(login): Form<LoginRequest>,
) -> Result<Response, error::Error> {
    let mut conn = state.db()
        .begin()
        .await
        .context("failed to retrieve database connection")?;

    tracing::debug!("login recieved: {login:#?}");

    let maybe_user = sqlx::query("select * from users where username = ?1")
        .bind(&login.username)
        .fetch_optional(&mut *conn)
        .await
        .context("database error when searching for login username")?;

    let Some(found_user) = maybe_user else {
        let options = LoginPageOptions {
            prev: query.get_prev(),
            user_not_found: true,
            clear_session_id: true,
            ..Default::default()
        };

        return respond_login_page(&state, options)
    };

    let users_id: i64 = found_user.get(0);
    //let user_uid: String = found_user.get(1);
    //let username: String = found_user.get(2);
    let password: String = found_user.get(3);
    //let version: i64 = found_user.get(4);

    let argon_config = Argon2::default();
    let parsed_hash = match PasswordHash::new(&password) {
        Ok(hash) => hash,
        Err(err) => {
            tracing::debug!("argon2 PasswordHash error: {err:#?}");

            return Err(error::Error::context("failed to create argon2 password hash"));
        }
    };

    if let Err(err) = argon_config.verify_password(login.password.as_bytes(), &parsed_hash) {
        tracing::debug!("verify_password failed: {err:#?}");

        let options = LoginPageOptions {
            prev: query.get_prev(),
            invalid_password: true,
            clear_session_id: true,
            ..Default::default()
        };

        return respond_login_page(&state, options);
    }

    let mut options = SessionOptions::new(users_id);
    options.authenticated = true;
    options.verified = true;

    let session = Session::create(&mut conn, options)
        .await
        .context("failed to create session for login")?;

    let session_cookie = session.build_cookie();

    conn.commit()
        .await
        .context("failed to commit transaction for login")?;

    let location = query.get_prev()
        .unwrap_or("/".to_owned());

    Response::builder()
        .status(StatusCode::FOUND)
        .header("location", location)
        .header("set-cookie", session_cookie)
        .body(Body::empty())
        .context("failed to create login redirect response")
}
