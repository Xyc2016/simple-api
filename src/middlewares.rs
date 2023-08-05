use crate::context::Context;
use crate::middleware::Middleware;

pub use crate::types::HttpResonse;
use crate::utils;
use anyhow::Ok;
use async_trait::async_trait;

use hyper::{header, Body, Request};
pub struct SessionMiddleware;

#[async_trait]
impl Middleware for SessionMiddleware {
    async fn pre_process(
        &self,
        req: &mut Request<Body>,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<HttpResonse>> {
        let sp = match ctx.session_provider {
            Some(ref v) => v,
            None => return Ok(None),
        };

        let cookie_string = match req.headers().get(header::COOKIE) {
            Some(v) => v.to_str()?,
            None => "",
        };

        let cookie_map = utils::cookie::parse_cookie(cookie_string);
        let session = sp.open_session(&cookie_map).await?;
        ctx.session = session;
        Ok(None)
    }

    async fn post_process(
        &self,
        _req: &mut Request<Body>,
        res: &mut HttpResonse,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<HttpResonse>> {
        if let (Some(session), Some(sp)) = (&ctx.session, &ctx.session_provider) {
            sp.save_session(session, res).await?;
        }
        Ok(None)
    }
}
