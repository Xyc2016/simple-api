use std::sync::Arc;

use anyhow::{anyhow, Ok};
use async_trait::async_trait;
use hyper::{Body, Method, Request, StatusCode};
use redis::AsyncCommands;
use serde_json::json;
use simple_api::{
    context::Context,
    resp_build,
    types::ResT,
    view::{View, ViewHandler},
    SimpleApi,
};
use tokio::sync::Mutex;

struct CustomState {
    redis_conn: Mutex<redis::aio::Connection>,
}

struct Index;

#[async_trait]
impl ViewHandler for Index {
    async fn call(&self, req: &mut Request<Body>, ctx: &mut Context) -> anyhow::Result<ResT> {
        let s = ctx.get_state::<CustomState>()?;
        let CustomState { redis_conn } = s.as_ref();

        let mut conn = redis_conn.lock().await;
        conn.incr("a", 1).await?;
        let r: Option<String> = conn.get("a").await?;
        dbg!(r);
        let session = ctx.session.as_mut().ok_or(anyhow!("no ses"))?;

        let new_count = match session.get("count")? {
            Some(v) => json!(v.as_i64().unwrap() + 1),
            None => json!(0),
        };
        session.set("count", new_count)?;

        resp_build::ok_json(json!(
            {"Hello": "World!", "path": req.uri().path(),
            "session": session.value(),
        }
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
async fn main() -> anyhow::Result<()> {
    SimpleApi::add_route("/", View::new(vec![Method::GET], Box::new(Index))).await;
    SimpleApi::add_route(
        "/unauthed",
        View::new(vec![Method::GET], Box::new(Unauthed)),
    )
    .await;
    SimpleApi::set_state(Arc::new(CustomState {
        redis_conn: Mutex::new(
            redis::Client::open("redis://localhost:6379/10")
                .unwrap()
                .get_async_connection()
                .await?,
        ),
    }))
    .await;
    Ok(SimpleApi::run("127.0.0.1:5001").await)
}
