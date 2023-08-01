use hyper::{Body, Response};
use std::{any::Any, sync::Arc};
use std::collections::HashMap;

pub type ResT = Response<Body>;

pub type State = Arc<dyn Any + Send + Sync>;

pub type CookieMap = HashMap<String, String>;
