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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionApiRequest {
    pub method: String,
    pub path: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionApiResponse {
    pub status: u16,
    pub body: Option<String>,
}

#[async_trait]
pub trait ExtensionApiHandler: Send + Sync {
    async fn handle(&self, request: ExtensionApiRequest) -> Result<ExtensionApiResponse>;
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
    extension_apis: Arc<RwLock<HashMap<String, Arc<dyn ExtensionApiHandler>>>>,
}

fn validate_extension_namespace(namespace: &str) -> Result<()> {
    if namespace.trim().is_empty() {
        return Err(anyhow!("extension namespace cannot be empty"));
    }
    if namespace != namespace.trim() {
        return Err(anyhow!(
            "extension namespace cannot have leading or trailing whitespace"
        ));
    }
    Ok(())
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            extension_apis: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, plugin: Arc<dyn Plugin>) -> Result<()> {
        let manifest = plugin.manifest();
        let id = manifest.id.as_str();
        if id.trim().is_empty() {
            return Err(anyhow!("plugin id cannot be empty"));
        }
        if id != id.trim() {
            return Err(anyhow!(
                "plugin id cannot have leading or trailing whitespace"
            ));
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
        let plugin_list: Vec<Arc<dyn Plugin>> = {
            let plugins = self.plugins.read().await;
            plugins.values().cloned().collect()
        };
        let mut manifests = plugin_list
            .iter()
            .map(|plugin| plugin.manifest())
            .collect::<Vec<_>>();
        manifests.sort_by(|a, b| a.id.cmp(&b.id));
        manifests
    }

    pub async fn register_extension_api(
        &self,
        namespace: &str,
        handler: Arc<dyn ExtensionApiHandler>,
    ) -> Result<()> {
        validate_extension_namespace(namespace)?;

        let mut apis = self.extension_apis.write().await;
        if apis.contains_key(namespace) {
            return Err(anyhow!(
                "extension namespace '{}' is already registered",
                namespace
            ));
        }

        apis.insert(namespace.to_string(), handler);
        Ok(())
    }

    pub async fn list_extension_namespaces(&self) -> Vec<String> {
        let apis = self.extension_apis.read().await;
        let mut namespaces = apis.keys().cloned().collect::<Vec<_>>();
        namespaces.sort();
        namespaces
    }

    pub async fn dispatch_extension_api(
        &self,
        namespace: &str,
        request: ExtensionApiRequest,
    ) -> Result<ExtensionApiResponse> {
        validate_extension_namespace(namespace)?;

        let handler = {
            let apis = self.extension_apis.read().await;
            apis.get(namespace).cloned()
        }
        .ok_or_else(|| anyhow!("extension namespace '{}' is not registered", namespace))?;

        handler.handle(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPlugin {
        manifest: PluginManifest,
    }

    struct EchoExtensionApi;

    #[async_trait]
    impl Plugin for MockPlugin {
        fn manifest(&self) -> PluginManifest {
            self.manifest.clone()
        }
    }

    #[async_trait]
    impl ExtensionApiHandler for EchoExtensionApi {
        async fn handle(&self, request: ExtensionApiRequest) -> Result<ExtensionApiResponse> {
            Ok(ExtensionApiResponse {
                status: 200,
                body: Some(format!("{} {}", request.method, request.path)),
            })
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
    async fn rejects_plugin_id_with_surrounding_whitespace() {
        let registry = PluginRegistry::new();

        let err = registry
            .register(plugin(
                "  builtin.indexer.torznab  ",
                "Torznab",
                PluginCapability::Indexer,
            ))
            .await
            .expect_err("plugin id with surrounding whitespace must fail");

        assert!(
            err.to_string().contains("leading or trailing whitespace"),
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

    #[tokio::test]
    async fn registers_and_lists_extension_namespaces() {
        let registry = PluginRegistry::new();

        registry
            .register_extension_api("media.tools", Arc::new(EchoExtensionApi))
            .await
            .expect("register extension api");

        registry
            .register_extension_api("metadata.lookup", Arc::new(EchoExtensionApi))
            .await
            .expect("register second extension api");

        let namespaces = registry.list_extension_namespaces().await;
        assert_eq!(namespaces, vec!["media.tools", "metadata.lookup"]);
    }

    #[tokio::test]
    async fn rejects_extension_namespace_with_surrounding_whitespace() {
        let registry = PluginRegistry::new();

        let err = registry
            .register_extension_api("  media.tools  ", Arc::new(EchoExtensionApi))
            .await
            .expect_err("namespace with surrounding whitespace must fail");

        assert!(
            err.to_string().contains("leading or trailing whitespace"),
            "unexpected error: {err}"
        );
        assert_eq!(registry.list_extension_namespaces().await.len(), 0);
    }

    #[tokio::test]
    async fn dispatch_rejects_namespace_with_surrounding_whitespace() {
        let registry = PluginRegistry::new();

        registry
            .register_extension_api("media.tools", Arc::new(EchoExtensionApi))
            .await
            .expect("register extension api");

        let err = registry
            .dispatch_extension_api(
                "  media.tools  ",
                ExtensionApiRequest {
                    method: "GET".to_string(),
                    path: "/health".to_string(),
                    body: None,
                },
            )
            .await
            .expect_err("namespace with surrounding whitespace must fail dispatch");

        assert!(
            err.to_string().contains("leading or trailing whitespace"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn rejects_duplicate_extension_namespace() {
        let registry = PluginRegistry::new();

        registry
            .register_extension_api("media.tools", Arc::new(EchoExtensionApi))
            .await
            .expect("first registration succeeds");

        let err = registry
            .register_extension_api("media.tools", Arc::new(EchoExtensionApi))
            .await
            .expect_err("duplicate namespace must fail");

        assert!(
            err.to_string().contains("already registered"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn dispatches_to_registered_extension_namespace() {
        let registry = PluginRegistry::new();

        registry
            .register_extension_api("media.tools", Arc::new(EchoExtensionApi))
            .await
            .expect("register extension api");

        let response = registry
            .dispatch_extension_api(
                "media.tools",
                ExtensionApiRequest {
                    method: "GET".to_string(),
                    path: "/health".to_string(),
                    body: None,
                },
            )
            .await
            .expect("dispatch succeeds");

        assert_eq!(response.status, 200);
        assert_eq!(response.body.as_deref(), Some("GET /health"));
    }

    #[tokio::test]
    async fn dispatch_to_unknown_namespace_returns_error() {
        let registry = PluginRegistry::new();

        let err = registry
            .dispatch_extension_api(
                "unknown.namespace",
                ExtensionApiRequest {
                    method: "GET".to_string(),
                    path: "/health".to_string(),
                    body: None,
                },
            )
            .await
            .expect_err("unknown namespace must fail");

        assert!(
            err.to_string().contains("is not registered"),
            "unexpected error: {err}"
        );
    }
}
