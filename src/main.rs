use std::collections::HashMap;
use std::future::Future;
use hyper::{Body, Method, Request, Response, Server};


use simple_api::{View, GLOBAL_SIMPLE_API, ViewHandler, SimpleApi};

#[derive(Debug)]
struct Index;

#[async_trait::async_trait]
impl ViewHandler for Index {
    async fn call(&self, _req: Request<Body>) -> Response<Body> {
        dbg!(_req);
        Response::builder().body(Body::from("index")).unwrap()
    }
}


#[tokio::main]
async fn main() {
    {
        let mut api = GLOBAL_SIMPLE_API.lock().unwrap();
        api.add_route("/", View::new(vec![Method::GET], Box::new(Index)));
    }
    SimpleApi::run("127.0.0.1:5001").await;

}
