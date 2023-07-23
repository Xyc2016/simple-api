use hyper::{Body, Method, Request, StatusCode, Response};
use serde_json::json;
use async_trait::async_trait;
use simple_api::{ResT, SimpleApi, View, ViewHandler, Middleware, Context};

#[derive(Debug)]
struct Session(String); // dummy session

struct SessionMiddleware;
#[async_trait]
impl Middleware for SessionMiddleware {
    async fn pre_process(
        &self,
        req: &mut Request<Body>,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>> {
        let mut session = Session("dummy".to_string());
        ctx.set("session", session);
        Ok(None)
    }

    async fn post_process(
        &self,
        req: &mut Request<Body>,
        res: &mut Response<Body>,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>> {
        dbg!(ctx.get::<Session>("session"));
        Ok(None)
    }

}

struct Index;

#[async_trait]
impl ViewHandler for Index {
    async fn call(&self, req: &mut Request<Body>) -> anyhow::Result<ResT> {
        ResT::ok_json(json!(
            {"Hello": "World!", "path": req.uri().path()}
        ))
    }
}

struct Unauthed;
#[async_trait]
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

    SimpleApi::add_route("/", View::new(vec![Method::GET], Box::new(Index))).await;
    SimpleApi::add_route(
        "/unauthed",
        View::new(vec![Method::GET], Box::new(Unauthed)),
    ).await;
    SimpleApi::run("127.0.0.1:5001").await;
}
