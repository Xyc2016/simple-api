use crate::context::Context;
use crate::middleware::Middleware;
use crate::types::ResT;
use crate::view::View;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Server, StatusCode};
use once_cell::sync::Lazy;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod context;
pub mod middleware;
pub mod resp_build;
pub mod session;
pub mod view;

pub mod types;

pub static GLOBAL_SIMPLE_API_INSTANCE: Lazy<Mutex<SimpleApi>> =
    Lazy::new(|| Mutex::new(SimpleApi::new()));

pub async fn apply_middlewares_pre(
    req: &mut Request<Body>,
    ctx: &mut Context,
    middlewares: &Vec<Arc<dyn Middleware>>,
) -> anyhow::Result<Option<ResT>> {
    for m in middlewares.iter() {
        match m.pre_process(req, ctx).await {
            Ok(None) => continue,
            other => return other,
        }
    }
    Ok(None)
}

async fn app_core(mut req: Request<Body>) -> Result<ResT, Infallible> {
    let path = req.uri().path().to_string();
    let f = GLOBAL_SIMPLE_API_INSTANCE
        .lock()
        .await
        .routes
        .get_mut(path.as_str())
        .map(|v| v.clone());

    let mut ctx = Context::new();
    let middlewares = GLOBAL_SIMPLE_API_INSTANCE.lock().await.middlewares.clone();

    match apply_middlewares_pre(&mut req, &mut ctx, &middlewares.lock().await.borrow_mut()).await {
        Ok(None) => (),
        Ok(Some(v)) => return Ok(v),
        Err(e) => return Ok(resp_build::internal_server_error_resp(e).unwrap()),
    }

    match f {
        Some(v) => match v.handler.call(&mut req, &mut ctx).await {
            Ok(r) => Ok(r),
            Err(e) => Ok(resp_build::internal_server_error_resp(e).unwrap()),
        },
        None => Ok(resp_build::build_response(
            format!("Not found: {}", path),
            StatusCode::NOT_FOUND,
            "text/html",
        )
        .unwrap()),
    }
}

pub struct SimpleApi {
    routes: HashMap<String, Arc<View>>,
    middlewares: Arc<Mutex<Vec<Arc<dyn Middleware>>>>,
}

impl SimpleApi {
    pub fn new() -> Self {
        SimpleApi {
            routes: HashMap::new(),
            middlewares: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add_route(path: &str, view: View) {
        let mut api = GLOBAL_SIMPLE_API_INSTANCE.lock().await;
        api.routes.insert(path.to_string(), Arc::new(view));
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

    pub async fn add_middleware(m: Arc<dyn Middleware>) {
        let api = GLOBAL_SIMPLE_API_INSTANCE.lock().await;
        api.middlewares.lock().await.push(m);
    }
}
