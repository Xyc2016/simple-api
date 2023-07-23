use async_trait::async_trait;
use hyper::service::{make_service_fn, service_fn};
use hyper::{header, Body, Method, Request, Response, Server, StatusCode};
use once_cell::sync::Lazy;
use redis::Commands;
use serde_json::{Value, json};
use std::any::Any;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::convert::Infallible as Inffallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

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

impl TryInto<Response<Body>> for ResT {
    type Error = anyhow::Error;
    fn try_into(self) -> Result<Response<Body>, Self::Error> {
        match self {
            ResT::Json(sc, v) => response_json(v.to_string(), sc),
            ResT::Raw(v) => anyhow::Ok(v),
        }
    }
}

#[async_trait]
pub trait ViewHandler: Send + Sync {
    async fn call(&self, req: &mut Request<Body>, ctx: &mut Context) -> anyhow::Result<ResT>;
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

pub struct Context {
    inner: HashMap<String, Box<dyn Any + Send>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            inner: HashMap::new(),
        }
    }

    pub fn get<T: 'static + Send>(&self, key: &str) -> Option<&T> {
        self.inner.get(key).and_then(|v| v.downcast_ref::<T>())
    }

    pub fn set<T: 'static + Send>(&mut self, key: &str, value: T) {
        self.inner.insert(key.to_string(), Box::new(value));
    }
}

#[async_trait]
pub trait Middleware: Send + Sync {
    async fn pre_process(
        &self,
        req: &mut Request<Body>,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>>;

    async fn post_process(
        &self,
        req: &mut Request<Body>,
        res: &mut Response<Body>,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>>;
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
        .insert(header::CONTENT_TYPE, content_type.parse()?);
    Ok(r)
}

pub fn response_json(body_text: String, status_code: StatusCode) -> anyhow::Result<Response<Body>> {
    build_response(body_text, status_code, "application/json")
}

pub fn internal_server_error_resp(error: anyhow::Error) -> anyhow::Result<Response<Body>> {
    build_response(
        format!("Error: {}", error.to_string()),
        StatusCode::INTERNAL_SERVER_ERROR,
        "text/html",
    )
}

pub fn result_to_resp(
    resp_result: anyhow::Result<Response<Body>>,
) -> anyhow::Result<Response<Body>> {
    resp_result.or_else(|e| internal_server_error_resp(e))
}

pub fn result_to_resp_force(resp_result: anyhow::Result<Response<Body>>) -> Response<Body> {
    return result_to_resp(resp_result).unwrap();
}

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

async fn app_core(mut req: Request<Body>) -> Result<Response<Body>, Inffallible> {
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
        Ok(Some(v)) => return Ok(result_to_resp_force(v.try_into())),
        Ok(None) => (),
        Err(e) => return Ok(internal_server_error_resp(e).unwrap()),
    }

    match f {
        Some(v) => match v.handler.call(&mut req, &mut ctx).await {
            Ok(r) => match r {
                ResT::Json(sc, v) => Ok(response_json(v.to_string(), sc).unwrap()),
                ResT::Raw(v) => Ok(v),
            },
            Err(e) => Ok(internal_server_error_resp(e).unwrap()),
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
            Ok::<_, Inffallible>(service_fn(app_core))
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

#[derive(Debug)]
pub struct RedisSession {
    inner: Value,
    sid: String,
}

static NO_SUCH_USER: &'static str = "NO_SUCH_USER";

#[async_trait]
impl Session<String> for RedisSession {
    async fn get(&self, key: &str) -> anyhow::Result<Option<Value>> {
        Ok(self.inner.get(key).cloned())
    }

    async fn set(&self, key: &str, value: Value) -> anyhow::Result<()> {
        Ok(())
    }

    async fn open(sid: String) -> anyhow::Result<Self> {
        let client = redis::Client::open("redis://localhost:6379/10")?;
        let mut conn = client.get_connection()?;
        let ov: Option<String> = conn.hgetall(&sid)?;
        let v = ov.ok_or(anyhow::anyhow!(NO_SUCH_USER))?;
        Ok(RedisSession {
            inner: json!({"v": v}),
            sid: sid.clone(),
        })
    }
    async fn save(&self) -> anyhow::Result<()> {
        Ok(())
    }
}


#[async_trait]
pub trait Session<K>: Sized {
    async fn get(&self, key: &str) -> anyhow::Result<Option<Value>>;
    async fn set(&self, key: &str, value: Value) -> anyhow::Result<()>;
    async fn open(sid: K) -> anyhow::Result<Self>;
    async fn save(&self) -> anyhow::Result<()>;
}

pub struct SessionMiddleware;

#[async_trait]
impl Middleware for SessionMiddleware {
    async fn pre_process(
        &self,
        req: &mut Request<Body>,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>> {
        let sid = req.headers().get("Cookie").ok_or(anyhow::anyhow!("cant find cookie"))?.to_str()?;
        let session = RedisSession::open(sid.to_string()).await?;
        ctx.set("session", session);
        Ok(None)
    }

    async fn post_process(
        &self,
        req: &mut Request<Body>,
        res: &mut Response<Body>,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>> {
        let session = ctx.get::<RedisSession>("session").ok_or(anyhow::anyhow!("Unauthed"))?;
        session.save().await?;
        Ok(None)
    }
}