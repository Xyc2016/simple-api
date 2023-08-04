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
use types::State;

pub mod middlewares;
pub mod context;
pub mod middleware;
pub mod resp_build;
pub mod session;
pub mod types;
pub mod utils;
pub mod view;

pub static GLOBAL_SIMPLE_API_INSTANCE: Lazy<SimpleApi> = Lazy::new(|| SimpleApi::new());

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

pub async fn apply_middlewares_post(
    req: &mut Request<Body>,
    res: &mut ResT,
    ctx: &mut Context,
    middlewares: &Vec<Arc<dyn Middleware>>,
) -> anyhow::Result<Option<ResT>> {
    for m in middlewares.iter() {
        match m.post_process(req, res, ctx).await {
            Ok(None) => continue,
            other => return other,
        }
    }
    Ok(None)
}

async fn app_core(mut req: Request<Body>) -> Result<ResT, Infallible> {
    let path = req.uri().path().to_string();
    let (f, middlewares, mut ctx, state) = {
        let f = SimpleApi::routes()
            .lock()
            .await
            .get(path.as_str())
            .map(|v| v.clone());

        let sp = SimpleApi::session_provider().lock().await.clone();

        let state = SimpleApi::state().lock().await.clone();

        let ctx = Context::new(sp, state.clone());
        let middlewares = SimpleApi::middlewares();
        (f, middlewares, ctx, state)
    };

    match apply_middlewares_pre(&mut req, &mut ctx, &middlewares.lock().await.borrow_mut()).await {
        Ok(None) => (),
        Ok(Some(v)) => return Ok(v),
        Err(e) => return Ok(resp_build::internal_server_error_resp(e).unwrap()),
    }

    match f {
        Some(v) => match v.handler.call(&mut req, &mut ctx).await {
            Ok(r) => {
                let mut res = r;
                match apply_middlewares_post(
                    &mut req,
                    &mut res,
                    &mut ctx,
                    &middlewares.lock().await.borrow_mut(),
                )
                .await
                {
                    Ok(None) => (),
                    Ok(Some(v)) => return Ok(v),
                    Err(e) => return Ok(resp_build::internal_server_error_resp(e).unwrap()),
                }
                Ok(res)
            }
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
    routes: Arc<Mutex<HashMap<String, Arc<View>>>>,
    middlewares: Arc<Mutex<Vec<Arc<dyn Middleware>>>>,
    session_provider: Arc<Mutex<Option<Arc<dyn session::SessionProvider>>>>,
    state: Arc<Mutex<State>>,
}

impl SimpleApi {
    pub fn new() -> Self {
        let _middlewares: Vec<Arc<dyn Middleware>> =
            vec![Arc::new(middlewares::SessionMiddleware)];
        SimpleApi {
            routes: Arc::new(Mutex::new(HashMap::new())),
            middlewares: Arc::new(Mutex::new(_middlewares)),
            session_provider: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(Arc::new(()))),
        }
    }

    pub fn instance() -> &'static SimpleApi {
        &GLOBAL_SIMPLE_API_INSTANCE
    }

    pub fn routes() -> Arc<Mutex<HashMap<String, Arc<View>>>> {
        Self::instance().routes.clone()
    }

    pub fn middlewares() -> Arc<Mutex<Vec<Arc<dyn Middleware>>>> {
        Self::instance().middlewares.clone()
    }

    pub fn session_provider() -> Arc<Mutex<Option<Arc<dyn session::SessionProvider>>>> {
        Self::instance().session_provider.clone()
    }

    pub fn state() -> Arc<Mutex<State>> {
        Self::instance().state.clone()
    }

    pub async fn add_route(path: &str, view: View) {
        let routes = SimpleApi::routes();
        let mut routes = routes.lock().await;
        routes.insert(path.to_string(), Arc::new(view));
    }

    pub async fn run(addr: &str) -> () {
        let addr = addr.parse::<SocketAddr>().unwrap();

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
        let middlewares = SimpleApi::middlewares();
        let mut middlewares = middlewares.lock().await;
        middlewares.push(m);
    }

    pub async fn set_session_provider(provider: Arc<dyn session::SessionProvider>) {
        let sp = SimpleApi::session_provider();
        let mut session_provider = sp.lock().await;
        *session_provider = Some(provider);
    }

    pub async fn set_state(state: State) {
        let s = SimpleApi::state();
        let mut s = s.lock().await;
        *s = state;
    }
}
