use crate::{context::Context, types::HttpRequest};
use crate::types::HttpResonse;
use anyhow;
use async_trait::async_trait;

#[async_trait]
pub trait Middleware: Send + Sync {
    async fn pre_process(
        &self,
        req: &mut HttpRequest,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<HttpResonse>>;

    async fn post_process(
        &self,
        req: &mut HttpRequest,
        res: &mut HttpResonse,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<HttpResonse>>;
}
