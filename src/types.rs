use std::{any::Any, sync::Arc};
use hyper::{Body, Response};

pub type ResT = Response<Body>;

pub type State = Arc<dyn Any + Send + Sync>;
