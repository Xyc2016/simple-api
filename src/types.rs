use hyper::{Body, Response};
use std::collections::HashMap;
use std::{any::Any, sync::Arc};

pub type ResT = Response<Body>;

pub type State = Arc<dyn Any + Send + Sync>;

pub type CookieMap = HashMap<String, String>;
