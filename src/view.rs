use crate::context::Context;
use crate::types::ResT;
use hyper::{Body, Method, Request};
use async_trait::async_trait;


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