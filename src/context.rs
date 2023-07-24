use std::any::Any;
use std::collections::HashMap;

pub struct Context {
    inner: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            inner: HashMap::new(),
        }
    }

    pub fn get<T: 'static + Send>(&self, key: &str) -> Option<&T> {
        self.inner.get(key).and_then(|v| v.downcast_ref::<T>())
    }

    pub fn set<T: 'static + Send + Sync>(&mut self, key: &str, value: T) {
        self.inner.insert(key.to_string(), Box::new(value));
    }
}
