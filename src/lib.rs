use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server};
use std::convert::Infallible;

use async_trait::async_trait;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;

#[async_trait]
pub trait ViewHandler: Send + Sync {
    async fn call(&self, req: Request<Body>) -> Response<Body>;
}

pub struct View {
    pub methods: Vec<Method>,
    pub handler: Box<dyn ViewHandler>,
}

impl View {
    pub fn new(methods: Vec<Method>, handler: Box<dyn ViewHandler>) -> Self {
        View { methods, handler }
    }
}

pub static GLOBAL_SIMPLE_API: Lazy<Mutex<SimpleApi>> = Lazy::new(|| Mutex::new(SimpleApi::new()));

async fn app_core(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path();
    let f = GLOBAL_SIMPLE_API
        .lock()
        .unwrap()
        .routes
        .get_mut(path)
        .map(|v| v.clone());

    match f {
        Some(v) => {
            let res = v.handler.call(req).await;
            return Ok(res);
        }
        None => Response::builder()
            .status(404)
            .body(Body::from("Not Found"))
            .map_err(|_| panic!("response builder error")),
    }
}

pub struct SimpleApi {
    pub routes: HashMap<String, Arc<View>>,
}

impl SimpleApi {
    pub fn new() -> Self {
        SimpleApi {
            routes: HashMap::new(),
        }
    }

    pub fn add_route(&mut self, path: &str, view: View) {
        self.routes.insert(path.to_string(), Arc::new(view));
    }

    pub async fn run(addr: &str) -> () {
        // We'll bind to 127.0.0.1:3000
        let addr = addr.parse::<SocketAddr>().unwrap();

        // A `Service` is needed for every connection, so this
        // creates one from our `hello_world` function.
        let make_svc = make_service_fn(|_conn| async {
            // service_fn converts our function into a `Service`
            Ok::<_, Infallible>(service_fn(app_core))
        });

        let server = Server::bind(&addr).serve(make_svc);

        // Run this server for... forever!
        if let Err(e) = server.await {
            eprintln!("server error: {}", e);
        }
    }
}
