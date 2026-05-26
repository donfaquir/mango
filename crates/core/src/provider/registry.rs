//! Immutable provider registry. Built once at app startup and shared by `Arc`.
//! Per spec-15: MS2 does not need hot reload of provider impls; if that ever
//! becomes a requirement, swap the inner `Arc<HashMap<...>>` for an `RwLock`.

use std::collections::HashMap;
use std::sync::Arc;

use super::traits::ModelProvider;
use crate::error::{CoreError, Result};

#[derive(Default, Clone)]
pub struct ProviderRegistry {
    inner: Arc<HashMap<String, Arc<dyn ModelProvider>>>,
}

impl ProviderRegistry {
    pub fn builder() -> ProviderRegistryBuilder {
        ProviderRegistryBuilder::default()
    }

    pub fn get(&self, provider_id: &str) -> Result<Arc<dyn ModelProvider>> {
        self.inner.get(provider_id).cloned().ok_or_else(|| {
            CoreError::TaskEngine(format!("provider not registered: {provider_id}"))
        })
    }
}

#[derive(Default)]
pub struct ProviderRegistryBuilder {
    map: HashMap<String, Arc<dyn ModelProvider>>,
}

impl ProviderRegistryBuilder {
    pub fn register(mut self, id: impl Into<String>, p: Arc<dyn ModelProvider>) -> Self {
        self.map.insert(id.into(), p);
        self
    }

    pub fn build(self) -> ProviderRegistry {
        ProviderRegistry {
            inner: Arc::new(self.map),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::stub::StubProvider;
    use super::*;

    #[test]
    fn empty_registry_returns_task_engine_error() {
        let reg = ProviderRegistry::builder().build();
        match reg.get("missing") {
            Err(CoreError::TaskEngine(msg)) => {
                assert!(msg.contains("provider not registered"));
            }
            Ok(_) => panic!("expected TaskEngine error"),
            Err(e) => panic!("expected TaskEngine error, got {e}"),
        }
    }

    #[test]
    fn registered_provider_is_retrievable() {
        let reg = ProviderRegistry::builder()
            .register("stub", Arc::new(StubProvider::default()))
            .build();
        assert!(reg.get("stub").is_ok());
    }
}
