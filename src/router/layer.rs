use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::http::{Extensions, Request, StatusCode};
use pin_project::pin_project;
use tokio::time::Sleep;
use tower::{Layer, Service};

use crate::net::body::json_error_response;
use crate::net::Error;

type Counter = Arc<AtomicU64>;

#[derive(Debug, Clone)]
pub struct RequestId {
    id: u64,
}

impl RequestId {
    pub fn from_request<B>(req: &Request<B>) -> Option<&Self> {
        Self::from_extensions(req.extensions())
    }

    pub fn from_extensions(extensions: &Extensions) -> Option<&Self> {
        extensions.get()
    }

    pub fn id(&self) -> &u64 {
        &self.id
    }
}

#[derive(Debug, Clone)]
pub struct RIDService<S> {
    inner: S,
    counter: Counter,
}

impl<S> RIDService<S> {
    pub fn new(inner: S, counter: Counter) -> Self {
        RIDService { inner, counter }
    }
}

impl<S, B> Service<Request<B>> for RIDService<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);

        {
            let extensions = request.extensions_mut();
            extensions.insert(RequestId { id });
        }

        self.inner.call(request)
    }
}

#[derive(Debug, Clone)]
pub struct RIDLayer {
    counter: Counter,
}

impl RIDLayer {
    pub fn new() -> Self {
        RIDLayer {
            counter: Arc::new(AtomicU64::new(1)),
        }
    }
}

impl<S> Layer<S> for RIDLayer {
    type Service = RIDService<S>;

    fn layer(&self, service: S) -> Self::Service {
        RIDService::new(service, self.counter.clone())
    }
}

pub enum TimeoutError<E> {
    Service(E),
    Timeout,
}

impl<E> From<E> for TimeoutError<E> {
    fn from(e: E) -> Self {
        TimeoutError::Service(e)
    }
}

impl<E> From<TimeoutError<E>> for Error
where
    E: Into<Error>,
{
    fn from(err: TimeoutError<E>) -> Self {
        match err {
            TimeoutError::Service(e) => e.into(),
            TimeoutError::Timeout => Error::Defined {
                response: json_error_response(
                    StatusCode::REQUEST_TIMEOUT,
                    "RequestTimeout",
                    "the request took too long to execute",
                ),
                msg: None,
                src: None,
            },
        }
    }
}

#[pin_project]
pub struct TimeoutFuture<F> {
    #[pin]
    resposne: F,
    #[pin]
    sleep: Sleep,
}

impl<F, Response, Error> Future for TimeoutFuture<F>
where
    F: Future<Output = Result<Response, Error>>,
{
    type Output = Result<Response, TimeoutError<Error>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.resposne.poll(cx) {
            Poll::Ready(result) => {
                let result = result.map_err(Into::into);

                return Poll::Ready(result);
            }
            Poll::Pending => {}
        }

        match this.sleep.poll(cx) {
            Poll::Ready(()) => Poll::Ready(Err(TimeoutError::Timeout)),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Timeout<S> {
    inner: S,
    timeout: Duration,
}

impl<S> Timeout<S> {
    pub fn new(inner: S, timeout: Duration) -> Self {
        Timeout { inner, timeout }
    }
}

impl<S, Request> Service<Request> for Timeout<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Error = TimeoutError<S::Error>;
    type Future = TimeoutFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let resposne = self.inner.call(request);
        let sleep = tokio::time::sleep(self.timeout);

        TimeoutFuture { resposne, sleep }
    }
}

#[derive(Debug, Clone)]
pub struct TimeoutLayer {
    timeout: Duration,
}

impl TimeoutLayer {
    pub fn new(timeout: Duration) -> Self {
        TimeoutLayer { timeout }
    }
}

impl<S> Layer<S> for TimeoutLayer {
    type Service = Timeout<S>;

    fn layer(&self, service: S) -> Self::Service {
        Timeout::new(service, self.timeout)
    }
}
