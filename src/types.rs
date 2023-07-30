use hyper::{Body, Response};
use std::{any::Any, sync::Arc};

pub type ResT = Response<Body>;

pub type State = Arc<dyn Any + Send + Sync>;
