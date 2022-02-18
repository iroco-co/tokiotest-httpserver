use std::convert::Infallible;
use std::sync::Arc;
use futures::future::BoxFuture;
use hyper::{Body, Method, Request};
use crate::StatusCode;

pub type Response = hyper::Response<hyper::Body>;
pub type HandlerCallback = Arc<dyn Fn(Request<hyper::Body>) -> BoxFuture<'static, Result<Response, Infallible>> + Send + Sync>;

#[derive(Default, Clone)]
pub struct HandlerBuilder {
    path: String,
    method: Method,
    status_code: StatusCode
}

impl HandlerBuilder {
    pub fn new(path: &str) -> HandlerBuilder {
        HandlerBuilder {
            path: String::from(path),
            method: Method::GET,
            status_code: StatusCode::INTERNAL_SERVER_ERROR
        }
    }

    pub fn method(mut self, method: Method) -> HandlerBuilder {
        self.method = method;
        self
    }

    pub fn status_code(mut self, status_code: StatusCode) -> HandlerBuilder {
        self.status_code = status_code;
        self
    }

    pub fn build(self) -> HandlerCallback {
        Arc::new(|req: Request<Body>| { Box::pin(async move {
            Ok(hyper::Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty()).unwrap())
        })})
    }
}

pub async fn default_handle(_req: Request<Body>) ->  Result<Response, Infallible> {
    Ok(hyper::Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty()).unwrap())
}
