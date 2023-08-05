use crate::context::Context;
use crate::types::HttpResonse;
use async_trait::async_trait;
use hyper::{Body, Method, Request};
use regex::Regex;

#[async_trait]
pub trait View: Send + Sync {
    async fn call(&self, req: &mut Request<Body>, ctx: &mut Context)
        -> anyhow::Result<HttpResonse>;
    fn methods(&self) -> Vec<Method>;
    fn re_path(&self) -> Regex;
}
