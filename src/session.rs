use std::sync::Arc;

use crate::types::CookieMap;
pub use crate::types::ResT;
use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::Mac;
use hyper::header;
use redis::AsyncCommands;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::Sha256;
use std::any;
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
        let session = session
            .as_any()
            .downcast_ref::<RedisSession>()
            .ok_or(anyhow::anyhow!("downcast failed"))?;
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

// Below is CookieSessionProvider
#[derive(Debug)]
pub struct CookieSession {
    inner: Value,
}

impl CookieSession {
    pub fn new() -> Self {
        CookieSession { inner: json!({}) }
    }
    pub fn from_value(value: Value) -> Self {
        CookieSession { inner: value }
    }
}

#[async_trait]
impl Session for CookieSession {
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

pub struct CookieSessionProvider {
    key: hmac::Hmac<Sha256>,
}

impl CookieSessionProvider {
    pub fn new(key: hmac::Hmac<Sha256>) -> Self {
        CookieSessionProvider { key }
    }

    pub fn from_slice(slice: &[u8]) -> anyhow::Result<Self> {
        let key = hmac::Hmac::<Sha256>::new_from_slice(slice)?;
        Ok(CookieSessionProvider::new(key))
    }

    pub fn from_hex(hex: &str) -> anyhow::Result<Self> {
        Ok(CookieSessionProvider::from_slice(&hex::decode(hex)?)?)
    }
}

impl CookieSessionProvider {
    fn cookie_name(&self) -> &'static str {
        "signed_session"
    }
    fn separator(&self) -> &'static str {
        "."
    }
    fn new_session(&self) -> Box<dyn Session> {
        Box::new(CookieSession::new())
    }
}

#[async_trait]
impl SessionProvider for CookieSessionProvider {
    async fn open_session(
        &self,
        cookie_map: &CookieMap,
    ) -> anyhow::Result<Option<Box<dyn Session>>> {
        let signed_session = match cookie_map.get(self.cookie_name()) {
            Some(v) => v,
            None => return Ok(Some(self.new_session())),
        };
        let v = signed_session
            .split(self.separator())
            .collect::<Vec<&str>>();
        let (session, signature) = match v.as_slice() {
            [session_b6, signature_b6] => (b6_to_s(session_b6)?, b6_to_vu8(&signature_b6)?),
            _ => return Err(anyhow::anyhow!("Signed session format is invalid")),
        };

        let signature_actual = get_signature(&self.key, &session);
        if signature != signature_actual {
            return Err(anyhow::anyhow!("Signature is invalid"));
        }

        Ok(Some(Box::new(CookieSession::from_value(
            serde_json::from_str(session.as_str())?,
        ))))
    }

    async fn save_session(&self, session: &Box<dyn Session>, res: &mut ResT) -> anyhow::Result<()> {
        let session_value = serde_json::to_string(session.value())?;
        let sigature = get_signature(&self.key, &session_value);
        let cookie_value = format!(
            "{}{}{}",
            s_to_b6(&session_value),
            self.separator(),
            vu8_to_b6(&sigature)
        );
        let cookie = cookie::Cookie::new(self.cookie_name(), cookie_value);
        res.headers_mut().append(
            header::SET_COOKIE,
            header::HeaderValue::from_str(cookie.to_string().as_str())?,
        );
        Ok(())
    }
}

fn s_to_b6(s: &str) -> String {
    URL_SAFE_NO_PAD.encode(s.as_bytes())
}

fn vu8_to_b6(v: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(v)
}

fn b6_to_vu8(s: &str) -> anyhow::Result<Vec<u8>> {
    Ok(URL_SAFE_NO_PAD.decode(s.as_bytes())?)
}

fn b6_to_s(s: &str) -> anyhow::Result<String> {
    let json_bytes = URL_SAFE_NO_PAD.decode(s.as_bytes())?;
    Ok(String::from_utf8(json_bytes)?)
}

pub fn get_signature(key: &hmac::Hmac<Sha256>, session: &str) -> Vec<u8> {
    let mut key = key.clone();
    key.update(session.as_bytes());
    let signature = key.finalize().into_bytes();
    signature.to_vec()
}
