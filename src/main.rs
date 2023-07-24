use std::sync::Arc;

use async_trait::async_trait;
use hyper::{Body, Method, Request, StatusCode};
use serde_json::json;
use simple_api::{
    context::Context,
    resp_build,
    session::{RedisSession, SessionMiddleware},
    types::ResT,
    view::{View, ViewHandler},
    SimpleApi,
};

struct Index;

#[async_trait]
impl ViewHandler for Index {
    async fn call(&self, req: &mut Request<Body>, ctx: &mut Context) -> anyhow::Result<ResT> {
        let session = ctx
            .get_mut::<RedisSession>("session").unwrap();
        session.inner["count"] = match session.inner.get("count") {
            Some(v) => json!(v.as_i64().unwrap() + 1),
            None => json!(0),
        };
        resp_build::ok_json(json!(
            {"Hello": "World!", "path": req.uri().path(), "session": session.inner}
        ))
    }
}

struct Unauthed;
#[async_trait]
impl ViewHandler for Unauthed {
    async fn call(&self, req: &mut Request<Body>, ctx: &mut Context) -> anyhow::Result<ResT> {
        resp_build::ret_json(
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
    )
    .await;
    SimpleApi::add_middleware(Arc::new(SessionMiddleware)).await;
    SimpleApi::run("127.0.0.1:5001").await;
}
