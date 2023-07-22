use hyper::service::{make_service_fn, service_fn};
use hyper::{header, Body, Method, Request, Response, Server, StatusCode};
use std::convert::Infallible as Inffallible;

use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;

pub static GLOBAL_SIMPLE_API_INSTANCE: Lazy<Mutex<SimpleApi>> =
    Lazy::new(|| Mutex::new(SimpleApi::new()));

pub enum ResT {
    Json(StatusCode, Value),
    Raw(Response<Body>),
}

impl ResT {
    pub fn ok_json(v: Value) -> anyhow::Result<ResT> {
        Self::ret_json(StatusCode::OK, v)
    }

    pub fn ret_json(status_code: StatusCode, v: Value) -> anyhow::Result<ResT> {
        anyhow::Ok(ResT::Json(status_code, v))
    }
}

#[async_trait]
pub trait ViewHandler: Send + Sync {
    async fn call(&self, req: &mut Request<Body>) -> anyhow::Result<ResT>;
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

pub fn build_response(
    body_text: String,
    status_code: StatusCode,
    content_type: &str,
) -> anyhow::Result<Response<Body>> {
    let mut r = Response::builder()
        .status(status_code)
        .body(Body::from(body_text))?;
    r.headers_mut()
        .insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    Ok(r)
}

pub fn response_json(body_text: String, status_code: StatusCode) -> anyhow::Result<Response<Body>> {
    build_response(body_text, status_code, "application/json")
}

async fn app_core(mut req: Request<Body>) -> Result<Response<Body>, Inffallible> {
    let path = req.uri().path().to_string();
    let f = GLOBAL_SIMPLE_API_INSTANCE
        .lock()
        .unwrap()
        .routes
        .get_mut(path.as_str())
        .map(|v| v.clone());

    match f {
        Some(v) => match v.handler.call(&mut req).await {
            Ok(r) => match r {
                ResT::Json(sc, v) => Ok(response_json(v.to_string(), sc).unwrap()),
                ResT::Raw(v) => Ok(v),
            },
            Err(e) => Ok(build_response(
                format!("Error: {}", e),
                StatusCode::INTERNAL_SERVER_ERROR,
                "text/html",
            )
            .unwrap()),
        },
        None => Ok(build_response(
            format!("Not found: {}", path),
            StatusCode::NOT_FOUND,
            "text/html",
        )
        .unwrap()),
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

    pub fn add_route(path: &str, view: View) {
        let mut api = GLOBAL_SIMPLE_API_INSTANCE.lock().unwrap();
        api.routes.insert(path.to_string(), Arc::new(view));
    }

    pub async fn run(addr: &str) -> () {
        // We'll bind to 127.0.0.1:3000
        let addr = addr.parse::<SocketAddr>().unwrap();

        // A `Service` is needed for every connection, so this
        // creates one from our `hello_world` function.
        let make_svc = make_service_fn(|_conn| async {
            // service_fn converts our function into a `Service`
            Ok::<_, Inffallible>(service_fn(app_core))
        });

        let server = Server::bind(&addr).serve(make_svc);

        // Run this server for... forever!
        if let Err(e) = server.await {
            eprintln!("server error: {}", e);
        }
    }
}
