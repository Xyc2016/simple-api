use crate::context::Context;
use crate::middleware::Middleware;

pub use crate::types::ResT;
use async_trait::async_trait;
use cookie::Cookie;
use hyper::{header, Body, Request};
pub struct SessionMiddleware;

#[async_trait]
impl Middleware for SessionMiddleware {
    async fn pre_process(
        &self,
        req: &mut Request<Body>,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>> {
        if ctx.session_provider.is_none() {
            return Ok(None);
        }
        let sp = ctx.session_provider.as_ref().unwrap();
        let sid: Option<String> = {
            match req.headers().get(header::COOKIE) {
                None => None,
                Some(v) => {
                    let mut session_id = None;
                    for cookie in Cookie::split_parse(v.to_str()?) {
                        let ck = cookie?;
                        let (name, value) = ck.name_value();
                        match name {
                            "session_id" => session_id = Some(value.to_owned()),
                            _ => continue,
                        }
                    }
                    session_id
                }
            }
        };

        match sid {
            Some(v) => {
                let old_session = sp.open_session(v).await?;
                let session = old_session.unwrap_or(sp.new_session().await?);
                ctx.session = Some(session);
                Ok(None)
            }
            None => {
                let session = sp.new_session().await?;
                ctx.session = Some(session);
                Ok(None)
            }
        }
    }

    async fn post_process(
        &self,
        req: &mut Request<Body>,
        res: &mut ResT,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>> {
        if let Some(session) = &ctx.session {
            let sid = session.sid();
            let cookie = Cookie::new("session_id", sid);
            res.headers_mut().append(
                header::SET_COOKIE,
                header::HeaderValue::from_str(cookie.to_string().as_str())?,
            );
            if let Some(sp) = &ctx.session_provider {
                sp.save_session(session.sid(), session.value()).await?;
            }
        }
        Ok(None)
    }
}
