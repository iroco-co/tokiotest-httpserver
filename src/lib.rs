use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use test_context::AsyncTestContext;
use tokio::sync::oneshot::{Receiver, Sender};
use tokio::task::JoinHandle;
use hyper::{Body, Client, Request, Server, StatusCode};
use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};

pub type Response = hyper::Response<hyper::Body>;
pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

struct HttpTestContext {
    client: Client<HttpConnector<GaiResolver>, Body>,
    server_handler: JoinHandle<Result<(), hyper::Error>>,
    sender: Sender<()>,
}

async fn handle(_req: Request<Body>) ->  Result<Response, Infallible> {
    Ok(hyper::Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty()).unwrap())
}

pub async fn run_service(
    addr: SocketAddr,
    rx: Receiver<()>
) -> impl Future<Output = Result<(), hyper::Error>> {
    let new_service = make_service_fn(move |_| {
        async {
            Ok::<_, Error>(service_fn(move |req| {
                handle(req)
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
        let addr = "127.0.0.1:12345".parse().expect("address creation works");
        let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
        let server_handler = tokio::spawn(run_service(addr, receiver).await);
        let client = Client::new();
        HttpTestContext {
            client,
            server_handler,
            sender,
        }
    }

    async fn teardown(self) {
        let _ = self.sender.send(()).unwrap();
        let _ = tokio::join!(self.server_handler);
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
        let resp = ctx.client.get("http://localhost:12345".parse::<Uri>().unwrap()).await.unwrap();
        assert_eq!(500, resp.status());
    }

}