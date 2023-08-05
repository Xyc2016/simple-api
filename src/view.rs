use crate::{context::Context, types::HttpRequest};
use crate::types::HttpResonse;
use async_trait::async_trait;
use hyper::{Method};
use regex::Regex;

#[async_trait]
pub trait View: Send + Sync {
    async fn call(&self, req: &mut HttpRequest, ctx: &mut Context)
        -> anyhow::Result<HttpResonse>;
    fn methods(&self) -> Vec<Method>;
    fn re_path(&self) -> Regex;
}
