// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

/// Represents the different hook points in the application lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptHookType {
    /// Executed when the application initializes.
    Initialization,
    /// Executed when the application shuts down.
    Shutdown,
    /// Executed before a search operation begins.
    BeforeSearch,
    /// Executed after a search operation completes.
    AfterSearch,
    /// Executed before an import operation begins.
    BeforeImport,
    /// Executed after an import operation completes.
    AfterImport,
    /// Executed when an error occurs.
    OnError,
}

impl ScriptHookType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Initialization => "initialization",
            Self::Shutdown => "shutdown",
            Self::BeforeSearch => "before_search",
            Self::AfterSearch => "after_search",
            Self::BeforeImport => "before_import",
            Self::AfterImport => "after_import",
            Self::OnError => "on_error",
        }
    }
}

/// Defines a custom script hook to be executed at specific lifecycle points.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptHookDefinition {
    /// Unique identifier for the hook.
    pub id: String,
    /// Type of hook this is.
    pub hook_type: ScriptHookType,
    /// Path to the script file to execute.
    pub script_path: PathBuf,
    /// Whether this hook is currently enabled.
    pub enabled: bool,
    /// Maximum execution time in seconds before the script is terminated.
    pub timeout_secs: u64,
    /// Searchable tags for this hook.
    pub tags: Vec<String>,
}

impl ScriptHookDefinition {
    /// Create a new script hook definition.
    pub fn new(
        hook_type: ScriptHookType,
        script_path: impl Into<PathBuf>,
        timeout_secs: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            hook_type,
            script_path: script_path.into(),
            enabled: true,
            timeout_secs,
            tags: Vec::new(),
        }
    }

    /// Add tags to this hook.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Enable or disable this hook.
    pub fn set_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Error type for script hook operations.
#[derive(Debug, Error)]
pub enum ScriptHookError {
    #[error("script not found: {0}")]
    ScriptNotFound(PathBuf),
    #[error("script timed out after {0} seconds")]
    Timeout(u64),
    #[error("script exited with code {0}: {1}")]
    NonZeroExit(i32, String),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("hook not found: {0}")]
    HookNotFound(String),
    #[error("hook with id {0} already exists")]
    DuplicateId(String),
}

pub type ScriptHookResult<T> = Result<T, ScriptHookError>;

/// Execution context for a script hook, containing environment variables and metadata.
#[derive(Debug, Clone)]
pub struct ScriptHookContext {
    pub event_type: String,
    pub timestamp: String,
    pub additional_vars: HashMap<String, String>,
}

impl ScriptHookContext {
    /// Create a new context for a script hook execution.
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            additional_vars: HashMap::new(),
        }
    }

    /// Add an additional environment variable to the context.
    pub fn with_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.additional_vars.insert(key.into(), value.into());
        self
    }
}

/// Executes script hooks with timeout management and error handling.
pub struct ScriptHookRunner;

impl ScriptHookRunner {
    /// Execute a script hook with the given context.
    ///
    /// Runs the script asynchronously and enforces `hook.timeout_secs`, killing the
    /// child process if the deadline is exceeded.  Reserved env vars (`EVENT_TYPE`,
    /// `HOOK_TYPE`, `TIMESTAMP`, `HOOK_ID`, `HOOK_TAGS`) are set **after** any
    /// caller-supplied `additional_vars` so they cannot be overridden.
    pub async fn execute(
        hook: &ScriptHookDefinition,
        context: &ScriptHookContext,
    ) -> ScriptHookResult<String> {
        if !hook.enabled {
            return Ok(String::new());
        }

        // Verify script exists
        if !hook.script_path.exists() {
            return Err(ScriptHookError::ScriptNotFound(hook.script_path.clone()));
        }

        // Build async command
        let mut cmd = tokio::process::Command::new(&hook.script_path);

        // Apply caller-supplied vars first …
        for (key, value) in &context.additional_vars {
            cmd.env(key, value);
        }
        // … then reserved vars so callers cannot override them.
        cmd.env("EVENT_TYPE", &context.event_type);
        cmd.env("HOOK_TYPE", hook.hook_type.as_str());
        cmd.env("TIMESTAMP", &context.timestamp);
        cmd.env("HOOK_ID", &hook.id);
        cmd.env("HOOK_TAGS", hook.tags.join(","));

        // Execute with timeout
        let duration = std::time::Duration::from_secs(hook.timeout_secs);
        let output = tokio::time::timeout(duration, cmd.output())
            .await
            .map_err(|_| ScriptHookError::Timeout(hook.timeout_secs))??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(ScriptHookError::NonZeroExit(
                output.status.code().unwrap_or(-1),
                stderr,
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Registry for managing script hooks.
pub struct ScriptHookRegistry {
    hooks: HashMap<String, ScriptHookDefinition>,
}

impl ScriptHookRegistry {
    /// Create a new empty script hook registry.
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// Register a new script hook.
    ///
    /// Returns [`ScriptHookError::DuplicateId`] if a hook with the same ID is already
    /// registered, preventing accidental overwrites.
    pub fn register(&mut self, hook: ScriptHookDefinition) -> ScriptHookResult<()> {
        if self.hooks.contains_key(&hook.id) {
            return Err(ScriptHookError::DuplicateId(hook.id.clone()));
        }
        self.hooks.insert(hook.id.clone(), hook);
        Ok(())
    }

    /// Get a hook by its ID.
    pub fn get(&self, id: &str) -> Option<&ScriptHookDefinition> {
        self.hooks.get(id)
    }

    /// Enable or disable a hook by ID.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> ScriptHookResult<()> {
        self.hooks
            .get_mut(id)
            .ok_or_else(|| ScriptHookError::HookNotFound(id.to_string()))?
            .enabled = enabled;
        Ok(())
    }

    /// Get all hooks of a specific type.
    pub fn by_type(&self, hook_type: ScriptHookType) -> Vec<&ScriptHookDefinition> {
        self.hooks
            .values()
            .filter(|h| h.hook_type == hook_type)
            .collect()
    }

    /// Get all enabled hooks of a specific type.
    pub fn enabled_by_type(&self, hook_type: ScriptHookType) -> Vec<&ScriptHookDefinition> {
        self.hooks
            .values()
            .filter(|h| h.hook_type == hook_type && h.enabled)
            .collect()
    }

    /// Get all hooks.
    pub fn list_all(&self) -> Vec<&ScriptHookDefinition> {
        self.hooks.values().collect()
    }

    /// Remove a hook by ID.
    pub fn remove(&mut self, id: &str) -> ScriptHookResult<ScriptHookDefinition> {
        self.hooks
            .remove(id)
            .ok_or_else(|| ScriptHookError::HookNotFound(id.to_string()))
    }

    /// Total number of hooks.
    pub fn count(&self) -> usize {
        self.hooks.len()
    }
}

impl Default for ScriptHookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_hook_type_as_str() {
        assert_eq!(ScriptHookType::Initialization.as_str(), "initialization");
        assert_eq!(ScriptHookType::BeforeSearch.as_str(), "before_search");
        assert_eq!(ScriptHookType::OnError.as_str(), "on_error");
    }

    #[test]
    fn create_script_hook_definition() {
        let hook =
            ScriptHookDefinition::new(ScriptHookType::BeforeSearch, "/path/to/script.sh", 30);
        assert_eq!(hook.hook_type, ScriptHookType::BeforeSearch);
        assert_eq!(hook.script_path, PathBuf::from("/path/to/script.sh"));
        assert_eq!(hook.timeout_secs, 30);
        assert!(hook.enabled);
    }

    #[test]
    fn hook_definition_with_tags() {
        let hook = ScriptHookDefinition::new(ScriptHookType::AfterImport, "/path/to/script.sh", 30)
            .with_tags(vec!["cleanup".to_string(), "logging".to_string()]);
        assert_eq!(hook.tags.len(), 2);
        assert!(hook.tags.contains(&"cleanup".to_string()));
    }

    #[test]
    fn hook_definition_disable_enable() {
        let hook =
            ScriptHookDefinition::new(ScriptHookType::Initialization, "/path/to/script.sh", 30)
                .set_enabled(false);
        assert!(!hook.enabled);
    }

    #[test]
    fn registry_register_and_get() {
        let mut registry = ScriptHookRegistry::new();
        let hook =
            ScriptHookDefinition::new(ScriptHookType::BeforeSearch, "/path/to/script.sh", 30);
        let hook_id = hook.id.clone();
        registry.register(hook).expect("register");

        let retrieved = registry.get(&hook_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().hook_type, ScriptHookType::BeforeSearch);
    }

    #[test]
    fn registry_rejects_duplicate_id() {
        let mut registry = ScriptHookRegistry::new();
        let hook =
            ScriptHookDefinition::new(ScriptHookType::BeforeSearch, "/path/to/script.sh", 30);
        let duplicate = ScriptHookDefinition {
            id: hook.id.clone(),
            hook_type: ScriptHookType::AfterSearch,
            script_path: PathBuf::from("/path/to/other.sh"),
            enabled: true,
            timeout_secs: 30,
            tags: vec![],
        };
        registry.register(hook).expect("first register");
        let err = registry
            .register(duplicate)
            .expect_err("duplicate should fail");
        assert!(matches!(err, ScriptHookError::DuplicateId(_)));
    }

    #[test]
    fn registry_by_type() {
        let mut registry = ScriptHookRegistry::new();
        registry
            .register(ScriptHookDefinition::new(
                ScriptHookType::BeforeSearch,
                "/path/to/script1.sh",
                30,
            ))
            .expect("register");
        registry
            .register(ScriptHookDefinition::new(
                ScriptHookType::AfterSearch,
                "/path/to/script2.sh",
                30,
            ))
            .expect("register");
        registry
            .register(ScriptHookDefinition::new(
                ScriptHookType::BeforeSearch,
                "/path/to/script3.sh",
                30,
            ))
            .expect("register");

        let before_search = registry.by_type(ScriptHookType::BeforeSearch);
        assert_eq!(before_search.len(), 2);

        let after_search = registry.by_type(ScriptHookType::AfterSearch);
        assert_eq!(after_search.len(), 1);
    }

    #[test]
    fn registry_enabled_by_type() {
        let mut registry = ScriptHookRegistry::new();
        let hook1 =
            ScriptHookDefinition::new(ScriptHookType::BeforeSearch, "/path/to/script1.sh", 30);
        let hook1_id = hook1.id.clone();

        let hook2 =
            ScriptHookDefinition::new(ScriptHookType::BeforeSearch, "/path/to/script2.sh", 30)
                .set_enabled(false);

        registry.register(hook1).expect("register");
        registry.register(hook2).expect("register");

        let enabled = registry.enabled_by_type(ScriptHookType::BeforeSearch);
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, hook1_id);
    }

    #[test]
    fn registry_set_enabled() {
        let mut registry = ScriptHookRegistry::new();
        let hook = ScriptHookDefinition::new(ScriptHookType::Shutdown, "/path/to/script.sh", 30);
        let hook_id = hook.id.clone();
        registry.register(hook).expect("register");

        registry.set_enabled(&hook_id, false).expect("set_enabled");
        assert!(!registry.get(&hook_id).unwrap().enabled);

        registry.set_enabled(&hook_id, true).expect("set_enabled");
        assert!(registry.get(&hook_id).unwrap().enabled);
    }

    #[test]
    fn registry_remove() {
        let mut registry = ScriptHookRegistry::new();
        let hook =
            ScriptHookDefinition::new(ScriptHookType::Initialization, "/path/to/script.sh", 30);
        let hook_id = hook.id.clone();
        registry.register(hook).expect("register");

        assert_eq!(registry.count(), 1);
        let removed = registry.remove(&hook_id);
        assert!(removed.is_ok());
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn registry_list_all() {
        let mut registry = ScriptHookRegistry::new();
        registry
            .register(ScriptHookDefinition::new(
                ScriptHookType::BeforeSearch,
                "/path/to/script1.sh",
                30,
            ))
            .expect("register");
        registry
            .register(ScriptHookDefinition::new(
                ScriptHookType::AfterImport,
                "/path/to/script2.sh",
                30,
            ))
            .expect("register");

        let all = registry.list_all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn script_hook_context_creation() {
        let ctx = ScriptHookContext::new("test_event");
        assert_eq!(ctx.event_type, "test_event");
        assert!(!ctx.timestamp.is_empty());
    }

    #[test]
    fn script_hook_context_with_vars() {
        let ctx = ScriptHookContext::new("test_event")
            .with_var("key1", "value1")
            .with_var("key2", "value2");
        assert_eq!(ctx.additional_vars.len(), 2);
        assert_eq!(ctx.additional_vars.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn hook_not_found_error() {
        let registry = ScriptHookRegistry::new();
        let err = registry.get("nonexistent");
        assert!(err.is_none());
    }

    #[test]
    fn registry_count() {
        let mut registry = ScriptHookRegistry::new();
        assert_eq!(registry.count(), 0);
        registry
            .register(ScriptHookDefinition::new(
                ScriptHookType::BeforeSearch,
                "/path/to/script.sh",
                30,
            ))
            .expect("register");
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn hook_definition_serialize_deserialize() {
        let hook =
            ScriptHookDefinition::new(ScriptHookType::BeforeSearch, "/path/to/script.sh", 30)
                .with_tags(vec!["tag1".to_string()]);

        let json = serde_json::to_string(&hook).expect("serialize");
        let deserialized: ScriptHookDefinition = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(hook, deserialized);
    }

    #[test]
    fn script_hook_type_serialize_deserialize() {
        let hook_type = ScriptHookType::AfterImport;
        let json = serde_json::to_string(&hook_type).expect("serialize");
        let deserialized: ScriptHookType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(hook_type, deserialized);
    }
}
