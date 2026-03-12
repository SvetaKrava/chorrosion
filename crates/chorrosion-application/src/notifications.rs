// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEventKind {
    WantedAlbumSearchTriggered,
    ReleaseMatched,
    DownloadCompleted,
    ImportFailed,
    Test,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationEvent {
    pub kind: NotificationEventKind,
    pub title: String,
    pub body: String,
    pub occurred_at: DateTime<Utc>,
}

impl NotificationEvent {
    pub fn test() -> Self {
        Self {
            kind: NotificationEventKind::Test,
            title: "Notification test event".to_string(),
            body: "This is a test notification from Chorrosion".to_string(),
            occurred_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationProviderKind {
    Email,
    Discord,
    Slack,
    Pushover,
    Script,
    Noop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationProviderConfig {
    pub kind: NotificationProviderKind,
    pub enabled: bool,
}

#[async_trait]
pub trait NotificationProvider: Send + Sync {
    fn kind(&self) -> NotificationProviderKind;
    fn enabled(&self) -> bool;
    async fn send(&self, event: &NotificationEvent) -> Result<()>;
}

pub struct NoopNotificationProvider;

#[async_trait]
impl NotificationProvider for NoopNotificationProvider {
    fn kind(&self) -> NotificationProviderKind {
        NotificationProviderKind::Noop
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn send(&self, _event: &NotificationEvent) -> Result<()> {
        Ok(())
    }
}

pub struct NotificationPipeline {
    providers: Vec<Box<dyn NotificationProvider>>,
}

impl NotificationPipeline {
    pub fn new(providers: Vec<Box<dyn NotificationProvider>>) -> Self {
        Self { providers }
    }

    pub fn provider_configs(&self) -> Vec<NotificationProviderConfig> {
        self.providers
            .iter()
            .filter(|p| p.kind() != NotificationProviderKind::Noop)
            .map(|p| NotificationProviderConfig {
                kind: p.kind(),
                enabled: p.enabled(),
            })
            .collect()
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    pub async fn dispatch(&self, event: &NotificationEvent) -> Result<usize> {
        let mut dispatched = 0usize;
        for provider in &self.providers {
            if !provider.enabled() {
                continue;
            }
            provider.send(event).await?;
            dispatched += 1;
        }
        Ok(dispatched)
    }
}

impl Default for NotificationPipeline {
    fn default() -> Self {
        Self {
            providers: vec![Box::new(NoopNotificationProvider)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DisabledProvider;

    #[async_trait]
    impl NotificationProvider for DisabledProvider {
        fn kind(&self) -> NotificationProviderKind {
            NotificationProviderKind::Email
        }

        fn enabled(&self) -> bool {
            false
        }

        async fn send(&self, _event: &NotificationEvent) -> Result<()> {
            panic!("disabled provider should not send");
        }
    }

    #[tokio::test]
    async fn dispatch_counts_enabled_providers() {
        let pipeline = NotificationPipeline::new(vec![
            Box::new(NoopNotificationProvider),
            Box::new(DisabledProvider),
        ]);
        // Both providers are disabled (noop is always disabled, DisabledProvider explicitly so)
        let sent = pipeline.dispatch(&NotificationEvent::test()).await.unwrap();
        assert_eq!(sent, 0);
    }

    #[test]
    fn provider_configs_reflect_pipeline() {
        let pipeline = NotificationPipeline::new(vec![
            Box::new(NoopNotificationProvider),
            Box::new(DisabledProvider),
        ]);
        let configs = pipeline.provider_configs();
        // Noop provider is filtered from configs; only real providers are included
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].kind, NotificationProviderKind::Email);
        assert!(!configs[0].enabled);
    }
}
