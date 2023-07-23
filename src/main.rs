use std::sync::Arc;

use hyper::{Body, Method, Request, StatusCode, Response};
use serde_json::json;
use async_trait::async_trait;
use simple_api::{ResT, SimpleApi, View, ViewHandler, SessionMiddleware, Context, RedisSession};


struct Index;

#[async_trait]
impl ViewHandler for Index {
    async fn call(&self, req: &mut Request<Body>, ctx: &mut Context) -> anyhow::Result<ResT> {
        let session = ctx.get::<RedisSession>("session").ok_or(anyhow::anyhow!("Unauthed"))?;
        dbg!(session);
        ResT::ok_json(json!(
            {"Hello": "World!", "path": req.uri().path()}
        ))
    }
}

struct Unauthed;
#[async_trait]
impl ViewHandler for Unauthed {
    async fn call(&self, req: &mut Request<Body>, ctx: &mut Context) -> anyhow::Result<ResT> {
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

    SimpleApi::add_route("/", View::new(vec![Method::GET], Box::new(Index))).await;
    SimpleApi::add_route(
        "/unauthed",
        View::new(vec![Method::GET], Box::new(Unauthed)),
    ).await;
    SimpleApi::add_middleware(Arc::new(SessionMiddleware)).await;
    SimpleApi::run("127.0.0.1:5001").await;
}
