use std::sync::{Arc, Mutex};

use crate::types::CookieMap;
pub use crate::types::ResT;
use async_trait::async_trait;
use redis::AsyncCommands;
use serde_json::{json, Value};
use uuid::Uuid;
use std::any;

static EMPTY_STRING: String = String::new();

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
const ENCRYPTED_SESSION_COOKIE_NAME: &'static str = "encrypted_session";

#[async_trait]
impl Session for RedisSession {
    fn get(&self, key: &str) -> anyhow::Result<Option<Value>> {
        Ok(self.inner.get(key).cloned())
    }

    fn set(&mut self, key: &str, value: Value) -> anyhow::Result<()> {
        Ok(self.inner[key] = value)
    }

    fn value(&self) -> &Value {
        &self.inner
    }
    fn as_any(&self) -> &dyn any::Any {
        self
    }
}

#[async_trait]
pub trait Session: Send + Sync + 'static + any::Any {
    fn get(&self, key: &str) -> anyhow::Result<Option<Value>>;
    fn set(&mut self, key: &str, value: Value) -> anyhow::Result<()>;
    fn value(&self) -> &Value;
    fn as_any(&self) -> &dyn any::Any;
}

#[async_trait]
pub trait SessionProvider: Send + Sync + 'static {
    // flask里SecureCookieSessionInterface是用的策略是：cookie里找不到session内容，或者session内容解码失败，都创建一个新的session
    // 这里也都参考这个来实现吧
    async fn open_session(
        &self,
        cookie_map: &CookieMap,
    ) -> anyhow::Result<Option<Box<dyn Session>>>;
    async fn save_session(&self, session: &Box<dyn Session>, res: &mut ResT) -> anyhow::Result<()>;
}

pub struct RedisSessionProvider {
    redis_cli: Arc<redis::Client>, // Better to use a connection pool
}

impl RedisSessionProvider {
    pub fn new(redis_cli: Arc<redis::Client>) -> Self {
        RedisSessionProvider { redis_cli }
    }

    fn new_session(&self) -> Box<dyn Session> {
        let sid = Uuid::new_v4().to_string();
        let session = RedisSession {
            inner: json!({}),
            sid,
        };
        Box::new(session)
    }
}

#[async_trait]
impl SessionProvider for RedisSessionProvider {
    async fn open_session(
        &self,
        cookie_map: &CookieMap,
    ) -> anyhow::Result<Option<Box<dyn Session>>> {
        let sid = match cookie_map.get("session_id") {
            Some(v) => v,
            None => return Ok(Some(self.new_session())),
        };
        let mut conn = self.redis_cli.get_async_connection().await?;
        let ov: Option<String> = conn.get(build_session_key(&sid)).await?;
        let serialized = ov.ok_or(anyhow::anyhow!(NO_SUCH_USER))?;
        Ok(Some(Box::new(RedisSession {
            inner: serde_json::from_str(serialized.as_str())?,
            sid: sid.to_string(),
        })))
    }

    async fn save_session(&self, session: &Box<dyn Session>, res: &mut ResT) -> anyhow::Result<()> {
        let session = session.as_any().downcast_ref::<RedisSession>().ok_or(anyhow::anyhow!("downcast failed"))?;
        let sid = session.sid.as_str();
        let value = session.value();
        let mut conn = self.redis_cli.get_async_connection().await?;
        let serialized = serde_json::to_string(value)?;
        conn.set(build_session_key(&sid), serialized).await?;

        let cookie = cookie::Cookie::new("session_id", sid.to_string());
        res.headers_mut().append(
            header::SET_COOKIE,
            header::HeaderValue::from_str(cookie.to_string().as_str())?,
        );

        Ok(())
    }
}
