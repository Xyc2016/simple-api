use std::sync::{Arc, Mutex};

pub use crate::types::ResT;
use async_trait::async_trait;
use hyper::{header, http::HeaderValue, HeaderMap};
use redis::AsyncCommands;
use serde_json::{json, Value};
use uuid::Uuid;

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
    async fn open_session(
        &self,
        sid: K,
        headers: Option<&HeaderMap<HeaderValue>>,
    ) -> anyhow::Result<Option<Box<dyn Session<String>>>>;
    async fn save_session(
        &self,
        sid: K,
        value: &Value,
        headers: Option<&mut HeaderMap<HeaderValue>>,
    ) -> anyhow::Result<()>;
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
        self.save_session(session.sid(), session.value(), None)
            .await?;
        Ok(Box::new(session))
    }

    async fn open_session(
        &self,
        sid: String,
        headers: Option<&HeaderMap<HeaderValue>>,
    ) -> anyhow::Result<Option<Box<dyn Session<String>>>> {
        let mut conn = self.redis_cli.get_async_connection().await?;
        let ov: Option<String> = conn.get(build_session_key(&sid)).await?;
        let serialized = ov.ok_or(anyhow::anyhow!(NO_SUCH_USER))?;
        Ok(Some(Box::new(RedisSession {
            inner: serde_json::from_str(serialized.as_str())?,
            sid,
        })))
    }

    async fn save_session(
        &self,
        sid: String,
        value: &Value,
        headers: Option<&mut HeaderMap<HeaderValue>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.redis_cli.get_async_connection().await?;
        let serialized = serde_json::to_string(value)?;
        conn.set(build_session_key(&sid), serialized).await?;
        Ok(())
    }
}

// Below is CookieSessionProvider
#[derive(Debug)]
pub struct CookieSession(Value);

#[async_trait]
impl Session<String> for CookieSession {
    fn get(&self, key: &str) -> anyhow::Result<Option<Value>> {
        Ok(self.0.get(key).cloned())
    }

    fn set(&mut self, key: &str, value: Value) -> anyhow::Result<()> {
        Ok(self.0[key] = value)
    }

    fn sid(&self) -> String {
        return EMPTY_STRING.clone();
    }
    fn value(&self) -> &Value {
        &self.0
    }
}

pub struct CookieSessionProvider(cookie::Key);

impl CookieSessionProvider {
    pub fn new(key: cookie::Key) -> Self {
        CookieSessionProvider(key)
    }

    pub fn from_hex(hex: &str) -> anyhow::Result<Self> {
        Ok(CookieSessionProvider(cookie::Key::from(&hex::decode(hex)?)))
    }

    pub fn key(&self) -> &cookie::Key {
        &self.0
    }
}

fn get_value_from_cookie_string(cookies_string: &str, key: &str) -> Option<String> {
    for cookie in cookie::Cookie::split_parse(cookies_string) {
        let ck = cookie.unwrap();
        let (name, value) = ck.name_value();
        match name {
            ENCRYPTED_SESSION_COOKIE_NAME => return Some(value.to_owned()),
            _ => continue,
        }
    }
    None
}

#[async_trait]
impl SessionProvider<String> for CookieSessionProvider {
    async fn new_session(&self) -> anyhow::Result<Box<dyn Session<String>>> {
        Ok(Box::new(CookieSession(json!({}))))
    }

    async fn open_session(
        &self,
        sid: String,
        headers: Option<&HeaderMap<HeaderValue>>,
    ) -> anyhow::Result<Option<Box<dyn Session<String>>>> {
        let headers = headers.ok_or(anyhow::anyhow!("headers must not be none"))?;
        let cookies_string = match headers.get(header::COOKIE) {
            None => return Err(anyhow::anyhow!(NO_COOKIE)),
            Some(v) => v.to_str()?,
        };

        let encrypted_session =
            get_value_from_cookie_string(cookies_string, ENCRYPTED_SESSION_COOKIE_NAME)
                .ok_or(anyhow::anyhow!("encrypted_session not found"))?;

        dbg!(&encrypted_session);

        let cookie_jar = cookie::CookieJar::new();

        let session_cookie = cookie_jar
            .private(self.key())
            .decrypt(cookie::Cookie::new(
                ENCRYPTED_SESSION_COOKIE_NAME,
                encrypted_session,
            ))
            .ok_or(anyhow::anyhow!("Can't decrypt cookie"))?;

        let session_string = session_cookie.value();
        let value: Value = serde_json::from_str(session_string)?;
        Ok(Some(Box::new(CookieSession(value))))
    }

    async fn save_session(
        &self,
        sid: String,
        value: &Value,
        headers: Option<&mut HeaderMap<HeaderValue>>,
    ) -> anyhow::Result<()> {
        let serialized = serde_json::to_string(value)?;

        let mut cookie_jar = cookie::CookieJar::new();

        cookie_jar.private_mut(self.key()).add(cookie::Cookie::new(
            ENCRYPTED_SESSION_COOKIE_NAME,
            serialized,
        ));
        let encrypted_cookie_content = cookie_jar
            .get(ENCRYPTED_SESSION_COOKIE_NAME)
            .ok_or(anyhow::anyhow!("Can't decrypt cookie"))?
            .value()
            .to_string();

        headers
            .ok_or(anyhow::anyhow!(
                "headers must not be none when using CookieSessionProvider"
            ))?
            .insert(
                header::SET_COOKIE,
                header::HeaderValue::from_str(
                    &cookie::Cookie::new(ENCRYPTED_SESSION_COOKIE_NAME, encrypted_cookie_content)
                        .to_string(),
                )?,
            );
        Ok(())
    }
}
