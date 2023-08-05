use hyper::{Body, Request, Response};
use std::collections::HashMap;
use std::{any::Any, sync::Arc};

pub type HttpResonse = Response<Body>;
pub type HttpRequest = Request<Body>;

pub type State = Arc<dyn Any + Send + Sync>; // It stores globals, such as database connection pool, etc.

pub type CookieMap = HashMap<String, String>;
pub type ViewPathArgs = HashMap<String, String>;
