use crate::context::Context;
use crate::types::ResT;
use anyhow;
use async_trait::async_trait;
use hyper::{Body, Request};

#[async_trait]
pub trait Middleware: Send + Sync {
    async fn pre_process(
        &self,
        req: &mut Request<Body>,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>>;

    async fn post_process(
        &self,
        req: &mut Request<Body>,
        res: &mut ResT,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>>;
}
