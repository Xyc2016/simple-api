use hyper::{Body, Method, Request, StatusCode};
use serde_json::json;

use simple_api::{ResT, SimpleApi, View, ViewHandler};

struct Index;

#[async_trait::async_trait]
impl ViewHandler for Index {
    async fn call(&self, req: &mut Request<Body>) -> anyhow::Result<ResT> {
        ResT::ok_json(json!(
            {"Hello": "World!", "path": req.uri().path()}
        ))
    }
}

struct Unauthed;
#[async_trait::async_trait]
impl ViewHandler for Unauthed {
    async fn call(&self, req: &mut Request<Body>) -> anyhow::Result<ResT> {
        ResT::ret_json(
            StatusCode::UNAUTHORIZED,
            json!(
                {"msg": "Unauthed", "path": req.uri().path()}
            ),
        )
    }
}

#[tokio::main]
async fn main() {
    SimpleApi::add_route("/", View::new(vec![Method::GET], Box::new(Index)));
    SimpleApi::add_route(
        "/unauthed",
        View::new(vec![Method::GET], Box::new(Unauthed)),
    );
    SimpleApi::run("127.0.0.1:5001").await;
}
