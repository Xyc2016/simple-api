use crate::context::Context;
use crate::middleware::Middleware;
use crate::types::HttpResonse;
use crate::view::View;
use hyper::StatusCode;
use hyper::{server::conn::http1, service::Service};

use tokio::net::TcpListener;

use route::match_view;

use hyper_util::rt::TokioIo;
use std::any;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use types::{HttpRequest, State};

pub mod context;
pub mod middleware;
pub mod middlewares;
pub mod response;
pub mod route;
pub mod session;
pub mod types;
pub mod utils;
pub mod view;
pub mod views;

pub async fn apply_middlewares_pre(
    req: &mut HttpRequest,
    ctx: &mut Context,
    middlewares: &Vec<Arc<dyn Middleware>>,
) -> anyhow::Result<Option<HttpResonse>> {
    for m in middlewares.iter() {
        match m.pre_process(req, ctx).await {
            Ok(None) => continue,
            other => return other,
        }
    }
    Ok(None)
}

pub async fn apply_middlewares_post(
    req: &mut HttpRequest,
    res: &mut HttpResonse,
    ctx: &mut Context,
    middlewares: &Vec<Arc<dyn Middleware>>,
) -> anyhow::Result<Option<HttpResonse>> {
    for m in middlewares.iter() {
        match m.post_process(req, res, ctx).await {
            Ok(None) => continue,
            other => return other,
        }
    }
    Ok(None)
}

async fn app_core(app: Arc<SimpleApi>, mut req: HttpRequest) -> anyhow::Result<HttpResonse> {
    let path = req.uri().path().to_string();
    let (view, mut ctx) = {
        let view_and_vpas = {
            let routes = app.routes();

            match_view(&routes, &path)
        };
        let (view, view_args) = match view_and_vpas {
            Some(v) => (Some(v.0), Some(v.1)),
            None => (None, None),
        };

        let sp = app.session_provider().clone();

        let state = app.state().clone();

        let ctx = Context::new(sp, state, view_args);
        (view, ctx)
    };

    let middlewares = app.middlewares();
    match apply_middlewares_pre(&mut req, &mut ctx, &middlewares).await {
        Ok(None) => (),
        Ok(Some(v)) => return Ok(v),
        Err(e) => return Ok(response::internal_server_error(e).unwrap()),
    }

    let Some(view) = view else {
        return Ok(response::build_response(
            format!("Not found: {}", path),
            StatusCode::NOT_FOUND,
            "text/html",
        )
        .unwrap());
    };

    match view.call(&mut req, &mut ctx).await {
        Ok(r) => {
            let mut res = r;
            match apply_middlewares_post(&mut req, &mut res, &mut ctx, &middlewares).await {
                Ok(None) => (),
                Ok(Some(v)) => return Ok(v),
                Err(e) => return Ok(response::internal_server_error(e).unwrap()),
            }
            Ok(res)
        }
        Err(e) => Ok(response::internal_server_error(e).unwrap()),
    }
}

pub struct SimpleApi {
    routes: Vec<Arc<dyn View>>,
    middlewares: Vec<Arc<dyn Middleware>>,
    session_provider: Option<Arc<dyn session::SessionProvider>>,
    state: State,
}

pub struct SimpleApiService {
    inner: Arc<SimpleApi>,
}

impl SimpleApiService {
    fn new(inner: Arc<SimpleApi>) -> Self {
        SimpleApiService { inner }
    }
}

impl SimpleApi {
    pub fn new() -> Self {
        let middlewares: Vec<Arc<dyn Middleware>> = vec![Arc::new(middlewares::SessionMiddleware)];
        SimpleApi {
            routes: Vec::new(),
            middlewares,
            session_provider: None,
            state: Arc::new(()),
        }
    }

    pub fn routes<'s>(&'s self) -> &'s Vec<Arc<dyn View>> {
        &self.routes
    }

    pub fn middlewares<'s>(&'s self) -> &'s Vec<Arc<dyn Middleware>> {
        &self.middlewares
    }

    pub fn session_provider<'s>(&'s self) -> &'s Option<Arc<dyn session::SessionProvider>> {
        &self.session_provider
    }

    pub fn state<'s>(&'s self) -> &'s State {
        &self.state
    }

    pub fn add_route<T: any::Any + View>(&mut self, view: T) {
        self.routes.push(Arc::new(view));
    }

    pub fn service(self: &Arc<Self>) -> SimpleApiService {
        SimpleApiService::new(self.clone())
    }

    pub async fn run(self, addr: &str) -> anyhow::Result<()> {
        let _self = Arc::new(self);
        let addr = addr.parse::<SocketAddr>().unwrap();
        let listener = TcpListener::bind(addr).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let service = _self.service();
            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }

        Ok(())
    }

    pub fn add_middleware(&mut self, m: Arc<dyn Middleware>) {
        self.middlewares.push(m);
    }

    pub async fn set_session_provider(&mut self, provider: Arc<dyn session::SessionProvider>) {
        self.session_provider = Some(provider);
    }

    pub fn set_state(&mut self, state: State) {
        self.state = state;
    }
}

impl Service<HttpRequest> for SimpleApiService {
    type Response = HttpResonse;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: HttpRequest) -> Self::Future {
        let res = app_core(self.inner.clone(), req);

        Box::pin(async { res.await })
    }
}
