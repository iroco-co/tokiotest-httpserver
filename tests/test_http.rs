use hyper::{StatusCode, Uri};
use tokiotest_httpserver::handler::HandlerBuilder;
use tokiotest_httpserver::HttpTestContext;
use test_context::test_context;

#[test_context(HttpTestContext)]
#[tokio::test]
async fn test_get_respond_200(ctx: &mut HttpTestContext) {
    let uri = format!("http://{}:{}/ok", "localhost", ctx.port).parse::<Uri>().unwrap();
    ctx.add(HandlerBuilder::new("/ok").status_code(StatusCode::OK).build());

    let resp = ctx.client.get(uri).await.unwrap();

    assert_eq!(200, resp.status());
}
