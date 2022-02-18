mod handler;

use std::collections::BinaryHeap;
use std::future::Future;
use std::net::{SocketAddr};
use test_context::AsyncTestContext;
use tokio::sync::oneshot::{Receiver, Sender};
use tokio::task::JoinHandle;
use hyper::{Body, Client, Server, StatusCode};
use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use lazy_static::lazy_static;
use std::sync::Mutex;
use std::sync::Arc;
use queues::{Queue, IsQueue, queue};
use crate::handler::{default_handle, HandlerCallback};

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
    port: u16,
    handlers: Arc<Mutex<Queue<HandlerCallback>>>
}

pub async fn run_service(addr: SocketAddr, rx: Receiver<()>,
    handlers: Arc<Mutex<Queue<HandlerCallback>>>) -> impl Future<Output = Result<(), hyper::Error>> {

    let new_service = make_service_fn(move |_| {
        let cloned_handlers = handlers.clone();
        async {
            Ok::<_, Error>(service_fn(move |req| {
                match cloned_handlers.lock() {
                    Ok(mut handlers_rw) => {
                        match handlers_rw.remove() {
                            Ok(handler) => { handler(req) }
                            Err(_err) => { Box::pin(default_handle(req)) }
                        }
                    }
                    Err(_err_lock) => Box::pin(default_handle(req))
                }
            }))
        }
    });
    let server = Server::bind(&addr).serve(new_service);

    server.with_graceful_shutdown(async { rx.await.ok(); })
}

#[async_trait::async_trait]
impl AsyncTestContext for HttpTestContext {
    async fn setup() -> HttpTestContext {
        let port = take_port();
        let addr = SocketAddr::new("127.0.0.1".parse().unwrap(), port);
        let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
        let handlers: Arc<Mutex<Queue<HandlerCallback>>> = Arc::new(Mutex::new(queue![]));
        let server_handler = tokio::spawn(run_service(addr, receiver, handlers.clone()).await);
        let client = Client::new();
        HttpTestContext {
            client,
            server_handler,
            sender,
            port,
            handlers
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
    use hyper::{Uri, StatusCode, Method, Request, Body};
    use crate::{HttpTestContext};
    use test_context::test_context;
    use queues::IsQueue;
    use crate::handler::HandlerBuilder;

    #[test_context(HttpTestContext)]
    #[tokio::test]
    async fn test_get_without_expect_should_send_500(ctx: &mut HttpTestContext) {
        let uri = format!("http://{}:{}", "localhost", ctx.port).parse::<Uri>().unwrap();
        let resp = ctx.client.get(uri).await.unwrap();
        assert_eq!(500, resp.status());
    }

    #[test_context(HttpTestContext)]
    #[tokio::test]
    async fn test_get_respond_404(ctx: &mut HttpTestContext) {
        let uri = format!("http://{}:{}/unknown", "localhost", ctx.port).parse::<Uri>().unwrap();
        ctx.handlers.lock().unwrap().add(HandlerBuilder::new("/unknown").status_code(StatusCode::NOT_FOUND).build()).unwrap();

        let resp = ctx.client.get(uri).await.unwrap();

        assert_eq!(404, resp.status());
    }

    #[test_context(HttpTestContext)]
    #[tokio::test]
    async fn test_get_endpoint(ctx: &mut HttpTestContext) {
        let uri = format!("http://{}:{}/foo", "localhost", ctx.port).parse::<Uri>().unwrap();
        ctx.handlers.lock().unwrap().add(HandlerBuilder::new("/foo").status_code(StatusCode::OK).build()).unwrap();

        let resp = ctx.client.get(uri).await.unwrap();

        assert_eq!(200, resp.status());
    }

    #[test_context(HttpTestContext)]
    #[tokio::test]
    async fn test_post_endpoint(ctx: &mut HttpTestContext) {
        let uri = format!("http://{}:{}/bar", "localhost", ctx.port).parse::<Uri>().unwrap();
        ctx.handlers.lock().unwrap().add(HandlerBuilder::new("/bar")
            .status_code(StatusCode::OK)
            .method(Method::POST).build()).unwrap();

        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .body(Body::from("foo=bar"))
            .expect("request builder");

        let resp = ctx.client.request(req).await.unwrap();

        assert_eq!(200, resp.status());
    }
}