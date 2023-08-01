use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    session::{Session, SessionProvider},
    types::State,
};

pub struct AnyMap(HashMap<String, Box<dyn Any + Send + Sync>>);

impl AnyMap {
    pub fn get<T: 'static + Send>(&self, key: &str) -> Option<&T> {
        self.0.get(key).and_then(|v| v.downcast_ref::<T>())
    }

    pub fn set<T: 'static + Send + Sync>(&mut self, key: &str, value: T) {
        self.0.insert(key.to_string(), Box::new(value));
    }

    pub fn get_mut<T: 'static + Send>(&mut self, key: &str) -> Option<&mut T> {
        self.0.get_mut(key).and_then(|v| v.downcast_mut::<T>())
    }
}

pub struct Context {
    pub any_map: AnyMap, // like flask g
    pub session: Option<Box<dyn Session>>,
    pub session_provider: Option<Arc<dyn SessionProvider>>,
    pub state: State,
}

impl Context {
    pub fn new(session_provider: Option<Arc<dyn SessionProvider>>, state: State) -> Self {
        Context {
            any_map: AnyMap(HashMap::new()),
            session: None,
            session_provider,
            state,
        }
    }

    pub fn get_state<T: 'static + Send + Sync>(&self) -> anyhow::Result<Arc<T>> {
        self.state
            .clone()
            .downcast::<T>()
            .map_err(|_| anyhow::anyhow!("cast failed"))
    }
}
