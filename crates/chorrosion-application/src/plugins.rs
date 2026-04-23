// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    Indexer,
    DownloadClient,
    MetadataProvider,
    NotificationProvider,
    ScriptHook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub capabilities: Vec<PluginCapability>,
}

#[async_trait]
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> PluginManifest;

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct PluginRegistry {
    plugins: Arc<RwLock<HashMap<String, Arc<dyn Plugin>>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, plugin: Arc<dyn Plugin>) -> Result<()> {
        let manifest = plugin.manifest();
        let id = manifest.id.trim();
        if id.is_empty() {
            return Err(anyhow!("plugin id cannot be empty"));
        }

        let mut plugins = self.plugins.write().await;
        if plugins.contains_key(id) {
            return Err(anyhow!("plugin with id '{}' is already registered", id));
        }

        plugins.insert(id.to_string(), plugin);
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Option<Arc<dyn Plugin>> {
        let plugins = self.plugins.read().await;
        plugins.get(id).cloned()
    }

    pub async fn contains(&self, id: &str) -> bool {
        let plugins = self.plugins.read().await;
        plugins.contains_key(id)
    }

    pub async fn count(&self) -> usize {
        let plugins = self.plugins.read().await;
        plugins.len()
    }

    pub async fn list_manifests(&self) -> Vec<PluginManifest> {
        let plugins = self.plugins.read().await;
        let mut manifests = plugins
            .values()
            .map(|plugin| plugin.manifest())
            .collect::<Vec<_>>();
        manifests.sort_by(|a, b| a.id.cmp(&b.id));
        manifests
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPlugin {
        manifest: PluginManifest,
    }

    #[async_trait]
    impl Plugin for MockPlugin {
        fn manifest(&self) -> PluginManifest {
            self.manifest.clone()
        }
    }

    fn plugin(id: &str, name: &str, capability: PluginCapability) -> Arc<dyn Plugin> {
        Arc::new(MockPlugin {
            manifest: PluginManifest {
                id: id.to_string(),
                name: name.to_string(),
                version: "1.0.0".to_string(),
                capabilities: vec![capability],
            },
        })
    }

    #[tokio::test]
    async fn register_and_lookup_plugin() {
        let registry = PluginRegistry::new();
        let p = plugin(
            "builtin.indexer.torznab",
            "Torznab",
            PluginCapability::Indexer,
        );

        registry.register(p).await.expect("register plugin");

        assert!(registry.contains("builtin.indexer.torznab").await);
        assert_eq!(registry.count().await, 1);

        let manifest = registry
            .get("builtin.indexer.torznab")
            .await
            .expect("plugin registered")
            .manifest();
        assert_eq!(manifest.name, "Torznab");
    }

    #[tokio::test]
    async fn rejects_duplicate_ids() {
        let registry = PluginRegistry::new();

        registry
            .register(plugin(
                "builtin.indexer.torznab",
                "Torznab",
                PluginCapability::Indexer,
            ))
            .await
            .expect("first registration succeeds");

        let err = registry
            .register(plugin(
                "builtin.indexer.torznab",
                "Torznab duplicate",
                PluginCapability::Indexer,
            ))
            .await
            .expect_err("duplicate id must fail");

        assert!(
            err.to_string().contains("already registered"),
            "unexpected error: {err}"
        );
        assert_eq!(registry.count().await, 1);
    }

    #[tokio::test]
    async fn rejects_empty_plugin_id() {
        let registry = PluginRegistry::new();

        let err = registry
            .register(plugin("  ", "Invalid", PluginCapability::ScriptHook))
            .await
            .expect_err("empty plugin id must fail");

        assert!(
            err.to_string().contains("cannot be empty"),
            "unexpected error: {err}"
        );
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn lists_manifests_sorted_by_id() {
        let registry = PluginRegistry::new();

        registry
            .register(plugin(
                "builtin.notification.discord",
                "Discord",
                PluginCapability::NotificationProvider,
            ))
            .await
            .expect("register notification plugin");

        registry
            .register(plugin(
                "builtin.indexer.torznab",
                "Torznab",
                PluginCapability::Indexer,
            ))
            .await
            .expect("register indexer plugin");

        let manifests = registry.list_manifests().await;

        assert_eq!(manifests.len(), 2);
        assert_eq!(manifests[0].id, "builtin.indexer.torznab");
        assert_eq!(manifests[1].id, "builtin.notification.discord");
    }
}
