use crate::context::Context;
use crate::middleware::Middleware;
pub use crate::types::ResT;
use async_trait::async_trait;
use cookie::Cookie;
use hyper::service::{make_service_fn, service_fn};
use hyper::{header, Body, Method, Request, Response, Server, StatusCode};
use once_cell::sync::Lazy;
use redis::{AsyncCommands, Commands};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug)]
pub struct RedisSession {
    inner: Value,
    sid: String,
}

fn build_rkey(v: &Vec<&str>) -> String {
    v.join(":")
}

fn build_session_key(session_id: &str) -> String {
    build_rkey(&vec![SESSION_PREFIX, session_id])
}

static NO_SUCH_USER: &'static str = "Can't find this session";
static NO_COOKIE: &'static str = "No cookie";
static NO_SESSION_ID: &'static str = "No session id";
static SESSION_PREFIX: &'static str = "session";

#[async_trait]
impl Session<String> for RedisSession {
    async fn get(&self, key: &str) -> anyhow::Result<Option<Value>> {
        Ok(self.inner.get(key).cloned())
    }

    async fn set(&mut self, key: &str, value: Value) -> anyhow::Result<()> {
        Ok(self.inner[key] = value)
    }

    async fn new() -> anyhow::Result<Self> {
        let sid = Uuid::new_v4().to_string();
        let session = RedisSession {
            inner: json!({}),
            sid,
        };
        session.save().await?;
        Ok(session)
    }

    async fn open(sid: String) -> anyhow::Result<Option<Self>> {
        let client = redis::Client::open("redis://localhost:6379/10")?;
        let mut conn = client.get_async_connection().await?;
        let ov: Option<String> = conn.get(build_session_key(&sid)).await?;
        let serialized = ov.ok_or(anyhow::anyhow!(NO_SUCH_USER))?;
        Ok(Some(RedisSession {
            inner: serde_json::from_str(serialized.as_str())?,
            sid,
        }))
    }
    async fn save(&self) -> anyhow::Result<()> {
        let serialized = serde_json::to_string(&self.inner)?;
        let client = redis::Client::open("redis://localhost:6379/10")?;
        let mut conn = client.get_async_connection().await?;
        conn.set(build_session_key(&self.sid), serialized).await?;
        Ok(())
    }
}

#[async_trait]
pub trait Session<K>: Sized {
    async fn new() -> anyhow::Result<Self>;
    async fn get(&self, key: &str) -> anyhow::Result<Option<Value>>;
    async fn set(&mut self, key: &str, value: Value) -> anyhow::Result<()>;
    async fn open(sid: K) -> anyhow::Result<Option<Self>>;
    async fn save(&self) -> anyhow::Result<()>;
}

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
                let session = RedisSession::open(v)
                    .await?
                    .unwrap_or(RedisSession::new().await?);
                ctx.set("session", session);
                Ok(None)
            }
            None => {
                let session = RedisSession::new().await?;
                ctx.set("session", session);
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
        if let Some(session) = ctx.get::<RedisSession>("session") {
            session.save().await?;

            let cookie = Cookie::new("session_id", session.sid.clone());
            req.headers_mut().append(
                header::SET_COOKIE,
                header::HeaderValue::from_str(cookie.to_string().as_str())?,
            );
        }
        Ok(None)
    }
}
