use crate::{context::Context, session::Session};
use crate::middleware::Middleware;
pub use crate::types::ResT;
use async_trait::async_trait;
use cookie::Cookie;
use hyper::{header, Body, Request};
use crate::session::RedisSession;
pub struct SessionMiddleware;

#[async_trait]
impl Middleware for SessionMiddleware {
    async fn pre_process(
        &self,
        req: &mut Request<Body>,
        ctx: &mut Context,
    ) -> anyhow::Result<Option<ResT>> {
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
                let old_session = RedisSession::open(v)
                    .await?;
                let session = old_session.unwrap_or(RedisSession::new().await?);
                ctx.session = Some(session);
                Ok(None)
            }
            None => {
                let session = RedisSession::new().await?;
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
            session.save().await?;
            let sid = session.sid();
            let cookie = Cookie::new("session_id", sid);
            res.headers_mut().append(
                header::SET_COOKIE,
                header::HeaderValue::from_str(cookie.to_string().as_str())?,
            );
        }
        Ok(None)
    }
}