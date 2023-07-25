use std::sync::{Arc, Mutex};

pub use crate::types::ResT;
use async_trait::async_trait;
use redis::AsyncCommands;
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
    fn get(&self, key: &str) -> anyhow::Result<Option<Value>> {
        Ok(self.inner.get(key).cloned())
    }

    fn set(&mut self, key: &str, value: Value) -> anyhow::Result<()> {
        Ok(self.inner[key] = value)
    }

    fn sid(&self) -> String {
        self.sid.clone()
    }
    fn value(&self) -> &Value {
        &self.inner
    }
}

#[async_trait]
pub trait Session<K>: Send + Sync + 'static {
    fn get(&self, key: &str) -> anyhow::Result<Option<Value>>;
    fn set(&mut self, key: &str, value: Value) -> anyhow::Result<()>;
    fn sid(&self) -> K;
    fn value(&self) -> &Value;
}

#[async_trait]
pub trait SessionProvider<K>: Send + Sync + 'static {
    async fn new_session(&self) -> anyhow::Result<Box<dyn Session<String>>>;
    async fn open_session(&self, sid: K) -> anyhow::Result<Option<Box<dyn Session<String>>>>;
    async fn save_session(&self, sid: K, value: &Value) -> anyhow::Result<()>;
}

pub struct RedisSessionProvider {
    redis_cli: Arc<redis::Client>, // Better to use a connection pool
}

impl RedisSessionProvider {
    pub fn new(redis_cli: Arc<redis::Client>) -> Self {
        RedisSessionProvider { redis_cli }
    }
}

#[async_trait]
impl SessionProvider<String> for RedisSessionProvider {
    async fn new_session(&self) -> anyhow::Result<Box<dyn Session<String>>> {
        let sid = Uuid::new_v4().to_string();
        let session = RedisSession {
            inner: json!({}),
            sid,
        };
        self.save_session(session.sid(), session.value()).await?;
        Ok(Box::new(session))
    }

    async fn open_session(&self, sid: String) -> anyhow::Result<Option<Box<dyn Session<String>>>> {
        let mut conn = self.redis_cli.get_async_connection().await?;
        let ov: Option<String> = conn.get(build_session_key(&sid)).await?;
        let serialized = ov.ok_or(anyhow::anyhow!(NO_SUCH_USER))?;
        Ok(Some(Box::new(RedisSession {
            inner: serde_json::from_str(serialized.as_str())?,
            sid,
        })))
    }

    async fn save_session(&self, sid: String, value: &Value) -> anyhow::Result<()> {
        let mut conn = self.redis_cli.get_async_connection().await?;
        let serialized = serde_json::to_string(value)?;
        conn.set(build_session_key(&sid), serialized).await?;
        Ok(())
    }
}
