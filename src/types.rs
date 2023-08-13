use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{body, Request, Response};
use std::collections::HashMap;
use std::{any::Any, sync::Arc};

pub type HttpResonse = Response<Full<Bytes>>;
pub type HttpRequest = Request<body::Incoming>;

pub type State = Arc<dyn Any + Send + Sync>; // It stores globals, such as database connection pool, etc.

pub type CookieMap = HashMap<String, String>;
pub type ViewPathArgs = HashMap<String, String>;
