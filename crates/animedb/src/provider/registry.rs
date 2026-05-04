use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::model::SourceName;
use crate::provider::Provider;

pub struct ProviderRegistry {
    providers: HashMap<SourceName, Arc<dyn Provider>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        self.providers.insert(provider.source(), provider);
    }

    pub fn get(&self, source: SourceName) -> Result<Arc<dyn Provider>> {
        self.providers
            .get(&source)
            .cloned()
            .ok_or_else(|| Error::Validation(format!("Provider not found for source: {}", source)))
    }

    pub fn iter(&self) -> impl Iterator<Item = Arc<dyn Provider>> + '_ {
        self.providers.values().cloned()
    }
}

pub fn default_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(crate::provider::AniListProvider::default()));
    registry.register(Arc::new(crate::provider::JikanProvider::default()));
    registry.register(Arc::new(crate::provider::KitsuProvider::default()));
    registry.register(Arc::new(crate::provider::TvmazeProvider::default()));
    registry.register(Arc::new(crate::provider::ImdbProvider::default()));
    registry
}
