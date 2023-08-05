use crate::context::Context;
use crate::middleware::Middleware;
use crate::types::HttpResonse;
use crate::view::View;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Server, StatusCode};
use once_cell::sync::Lazy;
use route::match_view;
use std::borrow::BorrowMut;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use types::{State, HttpRequest};
use std::any;

pub mod context;
pub mod middleware;
pub mod middlewares;
pub mod views;
pub mod response;
pub mod route;
pub mod session;
pub mod types;
pub mod utils;
pub mod view;

pub static GLOBAL_SIMPLE_API_INSTANCE: Lazy<SimpleApi> = Lazy::new(|| SimpleApi::new());

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

async fn app_core(mut req: HttpRequest) -> Result<HttpResonse, Infallible> {
    let path = req.uri().path().to_string();
    let (view, mut ctx) = {
        let view_and_vpas = {
            let _routes = SimpleApi::routes();
            let _routes = _routes.lock().await;

            match_view(&_routes, &path)
        };
        let (view, view_args) = match view_and_vpas {
            Some(v) => (Some(v.0), Some(v.1)),
            None => (None, None),
        };

        let sp = SimpleApi::session_provider().lock().await.clone();

        let state = SimpleApi::state().lock().await.clone();

        let ctx = Context::new(sp, state, view_args);
        (view, ctx)
    };
    
    let middlewares = SimpleApi::middlewares();
    match apply_middlewares_pre(&mut req, &mut ctx, &middlewares.lock().await.borrow_mut()).await {
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
                Err(e) => return Ok(response::internal_server_error(e).unwrap()),
            }
            Ok(res)
        }
        Err(e) => Ok(response::internal_server_error(e).unwrap()),
    }
}

pub struct SimpleApi {
    routes: Arc<Mutex<Vec<Arc<dyn View>>>>,
    middlewares: Arc<Mutex<Vec<Arc<dyn Middleware>>>>,
    session_provider: Arc<Mutex<Option<Arc<dyn session::SessionProvider>>>>,
    state: Arc<Mutex<State>>,
}

impl SimpleApi {
    pub fn new() -> Self {
        let _middlewares: Vec<Arc<dyn Middleware>> = vec![Arc::new(middlewares::SessionMiddleware)];
        SimpleApi {
            routes: Arc::new(Mutex::new(Vec::new())),
            middlewares: Arc::new(Mutex::new(_middlewares)),
            session_provider: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(Arc::new(()))),
        }
    }

    pub fn instance() -> &'static SimpleApi {
        &GLOBAL_SIMPLE_API_INSTANCE
    }

    pub fn routes() -> Arc<Mutex<Vec<Arc<dyn View>>>> {
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

    pub async fn add_route<T: any::Any + View>(view: T) {
        let routes = SimpleApi::routes();
        let mut routes = routes.lock().await;
        routes.push(Arc::new(view));
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
