use std::collections::BinaryHeap;
use std::convert::Infallible;
use std::future::Future;
use std::net::{SocketAddr};
use test_context::AsyncTestContext;
use tokio::sync::oneshot::{Receiver, Sender};
use tokio::task::JoinHandle;
use hyper::{Body, Client, Request, Server, StatusCode};
use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use lazy_static::lazy_static;
use std::sync::Mutex;
use std::sync::Arc;
use futures::future::BoxFuture;

pub type Response = hyper::Response<hyper::Body>;
pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

lazy_static! {
    static ref PORTS: Mutex<BinaryHeap<u16>> = Mutex::new(BinaryHeap::from((12300u16..12400u16).collect::<Vec<u16>>()));
}

fn take_port() -> u16 {
    PORTS.lock().unwrap().pop().unwrap()
}

fn release_port(port: u16) {
    PORTS.lock().unwrap().push(port)
}

struct HttpTestContext {
    client: Client<HttpConnector<GaiResolver>, Body>,
    server_handler: JoinHandle<Result<(), hyper::Error>>,
    sender: Sender<()>,
    port: u16
}

async fn handle(_req: Request<Body>) ->  Result<Response, Infallible> {
    Ok(hyper::Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty()).unwrap())
}

pub async fn run_service(
    addr: SocketAddr,
    rx: Receiver<()>,
    handle: Arc<dyn Fn(Request<Body>) -> BoxFuture<'static, Result<Response, Infallible>> + Send + Sync>
) -> impl Future<Output = Result<(), hyper::Error>> {
    let new_service = make_service_fn(move |_| {
        let cloned_handle = Arc::clone(&handle);
        async {
            Ok::<_, Error>(service_fn(move |req| {
                cloned_handle(req)
            }))
        }
    });
    let server = Server::bind(&addr).serve(new_service);

    server.with_graceful_shutdown(async {
        rx.await.ok();
    })
}

#[async_trait::async_trait]
impl AsyncTestContext for HttpTestContext {
    async fn setup() -> HttpTestContext {
        let port = take_port();
        let addr = SocketAddr::new("127.0.0.1".parse().unwrap(), port);
        let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
        let server_handler = tokio::spawn(run_service(addr, receiver, Arc::new(|req: Request<Body>| {Box::pin(handle(req))}) ).await);
        let client = Client::new();
        HttpTestContext {
            client,
            server_handler,
            sender,
            port
        }
    }

    async fn teardown(self) {
        let _ = self.sender.send(()).unwrap();
        let _ = tokio::join!(self.server_handler);
        release_port(self.port)
    }
}

#[cfg(test)]
mod test {
    use hyper::Uri;
    use crate::HttpTestContext;
    use test_context::test_context;

    #[test_context(HttpTestContext)]
    #[tokio::test]
    async fn test_get_without_expect_should_send_500(ctx: &mut HttpTestContext) {
        let uri = format!("http://{}:{}", "localhost", ctx.port).parse::<Uri>().unwrap();
        let resp = ctx.client.get(uri).await.unwrap();
        assert_eq!(500, resp.status());
    }
}