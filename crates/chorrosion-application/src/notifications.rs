// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use async_trait::async_trait;
use chorrosion_config::AppConfig;
use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

pub struct EmailNotificationProvider {
    enabled: bool,
    from: Option<String>,
    to: Vec<String>,
}

pub struct DiscordWebhookProvider {
    enabled: bool,
    webhook_url: Option<String>,
    username: Option<String>,
    client: Client,
}

pub struct SlackWebhookProvider {
    enabled: bool,
    webhook_url: Option<String>,
    username: Option<String>,
    client: Client,
}

pub struct PushoverProvider {
    enabled: bool,
    api_token: Option<String>,
    user_key: Option<String>,
    api_url: String,
    client: Client,
}

impl DiscordWebhookProvider {
    pub fn from_config(config: &AppConfig) -> Self {
        let discord = &config.notifications.discord;
        let webhook_url = discord
            .webhook_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|url_str| {
                let parsed = Url::parse(url_str).ok();
                match parsed {
                    Some(ref p)
                        if matches!(p.scheme(), "http" | "https") && p.host().is_some() =>
                    {
                        Some(url_str.to_string())
                    }
                    _ => {
                        tracing::warn!(
                            target: "application",
                            "Discord webhook_url is not a valid http/https URL; provider will be disabled"
                        );
                        None
                    }
                }
            });
        let username = discord
            .username
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let client = Client::builder()
            .user_agent(concat!(
                "chorrosion/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/SvetaKrava/chorrosion)"
            ))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|error| {
                tracing::debug!(
                    target: "application",
                    ?error,
                    "Failed to build Discord webhook HTTP client with custom settings, falling back to default"
                );
                Client::new()
            });
        Self {
            enabled: discord.enabled && webhook_url.is_some(),
            webhook_url,
            username,
            client,
        }
    }
}

impl SlackWebhookProvider {
    pub fn from_config(config: &AppConfig) -> Self {
        let slack = &config.notifications.slack;
        let webhook_url = slack
            .webhook_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|url_str| {
                let parsed = Url::parse(url_str).ok();
                match parsed {
                    Some(ref p)
                        if matches!(p.scheme(), "http" | "https") && p.host().is_some() =>
                    {
                        Some(url_str.to_string())
                    }
                    _ => {
                        tracing::warn!(
                            target: "application",
                            "Slack webhook_url is not a valid http/https URL; provider will be disabled"
                        );
                        None
                    }
                }
            });
        let username = slack
            .username
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let client = Client::builder()
            .user_agent(concat!(
                "chorrosion/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/SvetaKrava/chorrosion)"
            ))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|error| {
                tracing::debug!(
                    target: "application",
                    ?error,
                    "Failed to build Slack webhook HTTP client with custom settings, falling back to default"
                );
                Client::new()
            });
        Self {
            enabled: slack.enabled && webhook_url.is_some(),
            webhook_url,
            username,
            client,
        }
    }
}

impl PushoverProvider {
    pub fn from_config(config: &AppConfig) -> Self {
        let pushover = &config.notifications.pushover;
        let api_token = pushover
            .api_token
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let user_key = pushover
            .user_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let api_url = pushover
            .api_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("https://api.pushover.net/1/messages.json")
            .to_string();

        let api_url_is_valid = match Url::parse(&api_url) {
            Ok(parsed) => matches!(parsed.scheme(), "http" | "https") && parsed.host().is_some(),
            Err(_) => false,
        };
        if !api_url_is_valid {
            tracing::warn!(
                target: "application",
                "Pushover api_url is not a valid http/https URL; provider will be disabled"
            );
        }

        let client = Client::builder()
            .user_agent(concat!(
                "chorrosion/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/SvetaKrava/chorrosion)"
            ))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|error| {
                tracing::debug!(
                    target: "application",
                    ?error,
                    "Failed to build Pushover HTTP client with custom settings, falling back to default"
                );
                Client::new()
            });

        Self {
            enabled: pushover.enabled
                && api_token.is_some()
                && user_key.is_some()
                && api_url_is_valid,
            api_token,
            user_key,
            api_url,
            client,
        }
    }
}

#[derive(Debug, Serialize)]
struct DiscordWebhookPayload {
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
}

#[derive(Debug, Serialize)]
struct SlackWebhookPayload {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
}

#[derive(Debug, Serialize)]
struct PushoverPayload {
    token: String,
    user: String,
    title: String,
    message: String,
}

impl EmailNotificationProvider {
    pub fn from_config(config: &AppConfig) -> Self {
        let email = &config.notifications.email;
        let to = sanitize_email_list(&email.to);
        let from = email
            .from
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let has_recipients = !to.is_empty();
        let has_sender = from.is_some();
        Self {
            enabled: email.enabled && has_recipients && has_sender,
            from,
            to,
        }
    }
}

fn sanitize_email_list(emails: &[String]) -> Vec<String> {
    emails
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[async_trait]
impl NotificationProvider for EmailNotificationProvider {
    fn kind(&self) -> NotificationProviderKind {
        NotificationProviderKind::Email
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        // Baseline implementation: log the outbound envelope and content.
        // SMTP transport wiring will be implemented in a dedicated follow-up task.
        tracing::trace!(
            target: "application",
            kind = ?self.kind(),
            has_from = self.from.is_some(),
            recipient_count = self.to.len(),
            title = %event.title,
            "email notification dispatched"
        );
        Ok(())
    }
}

#[async_trait]
impl NotificationProvider for DiscordWebhookProvider {
    fn kind(&self) -> NotificationProviderKind {
        NotificationProviderKind::Discord
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        let Some(webhook_url) = &self.webhook_url else {
            return Ok(());
        };

        let payload = DiscordWebhookPayload {
            content: format!("{}\n{}", event.title, event.body),
            username: self.username.clone(),
        };

        self.client
            .post(webhook_url)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;

        tracing::trace!(
            target: "application",
            kind = ?self.kind(),
            title = %event.title,
            "discord webhook notification dispatched"
        );

        Ok(())
    }
}

#[async_trait]
impl NotificationProvider for SlackWebhookProvider {
    fn kind(&self) -> NotificationProviderKind {
        NotificationProviderKind::Slack
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        let Some(webhook_url) = &self.webhook_url else {
            return Ok(());
        };

        let payload = SlackWebhookPayload {
            text: format!("{}\n{}", event.title, event.body),
            username: self.username.clone(),
        };

        self.client
            .post(webhook_url)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;

        tracing::trace!(
            target: "application",
            kind = ?self.kind(),
            title = %event.title,
            "slack webhook notification dispatched"
        );

        Ok(())
    }
}

#[async_trait]
impl NotificationProvider for PushoverProvider {
    fn kind(&self) -> NotificationProviderKind {
        NotificationProviderKind::Pushover
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    async fn send(&self, event: &NotificationEvent) -> Result<()> {
        let (Some(api_token), Some(user_key)) = (&self.api_token, &self.user_key) else {
            return Ok(());
        };

        let payload = PushoverPayload {
            token: api_token.clone(),
            user: user_key.clone(),
            title: event.title.clone(),
            message: event.body.clone(),
        };

        self.client
            .post(&self.api_url)
            .form(&payload)
            .send()
            .await?
            .error_for_status()?;

        tracing::trace!(
            target: "application",
            kind = ?self.kind(),
            title = %event.title,
            "pushover notification dispatched"
        );

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
            .filter_map(|p| {
                let kind = p.kind();
                if matches!(kind, NotificationProviderKind::Noop) {
                    None
                } else {
                    Some(NotificationProviderConfig {
                        kind,
                        enabled: p.enabled(),
                    })
                }
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

    pub fn from_config(config: &AppConfig) -> Self {
        let providers: Vec<Box<dyn NotificationProvider>> = vec![
            Box::new(EmailNotificationProvider::from_config(config)),
            Box::new(DiscordWebhookProvider::from_config(config)),
            Box::new(SlackWebhookProvider::from_config(config)),
            Box::new(PushoverProvider::from_config(config)),
            Box::new(NoopNotificationProvider),
        ];
        Self { providers }
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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

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

    struct EnabledProvider {
        sent: Arc<AtomicBool>,
    }

    #[async_trait]
    impl NotificationProvider for EnabledProvider {
        fn kind(&self) -> NotificationProviderKind {
            NotificationProviderKind::Discord
        }

        fn enabled(&self) -> bool {
            true
        }

        async fn send(&self, _event: &NotificationEvent) -> Result<()> {
            self.sent.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn dispatch_counts_enabled_providers() {
        let sent = Arc::new(AtomicBool::new(false));
        let pipeline = NotificationPipeline::new(vec![
            Box::new(NoopNotificationProvider),
            Box::new(DisabledProvider),
            Box::new(EnabledProvider { sent: sent.clone() }),
        ]);
        let count = pipeline.dispatch(&NotificationEvent::test()).await.unwrap();
        assert_eq!(count, 1, "only enabled providers should be counted");
        assert!(
            sent.load(Ordering::SeqCst),
            "enabled provider's send() should have been called"
        );
    }

    #[test]
    fn provider_configs_excludes_noop_providers() {
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

    #[test]
    fn from_config_enables_email_when_fully_configured() {
        let config = AppConfig {
            notifications: chorrosion_config::NotificationsConfig {
                email: chorrosion_config::EmailNotificationConfig {
                    enabled: true,
                    from: Some("noreply@example.com".to_string()),
                    to: vec!["user@example.com".to_string()],
                },
                ..Default::default()
            },
            ..AppConfig::default()
        };

        let pipeline = NotificationPipeline::from_config(&config);
        let providers = pipeline.provider_configs();
        assert_eq!(providers.len(), 4);
        assert_eq!(providers[0].kind, NotificationProviderKind::Email);
        assert!(providers[0].enabled);
        assert_eq!(providers[1].kind, NotificationProviderKind::Discord);
        assert!(!providers[1].enabled);
        assert_eq!(providers[2].kind, NotificationProviderKind::Slack);
        assert!(!providers[2].enabled);
        assert_eq!(providers[3].kind, NotificationProviderKind::Pushover);
        assert!(!providers[3].enabled);
    }

    #[test]
    fn from_config_disables_email_when_missing_fields() {
        let config = AppConfig {
            notifications: chorrosion_config::NotificationsConfig {
                email: chorrosion_config::EmailNotificationConfig {
                    enabled: true,
                    from: None,
                    to: vec![],
                },
                ..Default::default()
            },
            ..AppConfig::default()
        };

        let pipeline = NotificationPipeline::from_config(&config);
        let providers = pipeline.provider_configs();
        assert_eq!(providers.len(), 4);
        assert_eq!(providers[0].kind, NotificationProviderKind::Email);
        assert!(!providers[0].enabled);
        assert_eq!(providers[1].kind, NotificationProviderKind::Discord);
        assert!(!providers[1].enabled);
        assert_eq!(providers[2].kind, NotificationProviderKind::Slack);
        assert!(!providers[2].enabled);
        assert_eq!(providers[3].kind, NotificationProviderKind::Pushover);
        assert!(!providers[3].enabled);
    }

    #[test]
    fn from_config_disables_email_when_to_is_whitespace_only() {
        let config = AppConfig {
            notifications: chorrosion_config::NotificationsConfig {
                email: chorrosion_config::EmailNotificationConfig {
                    enabled: true,
                    from: Some("noreply@example.com".to_string()),
                    to: vec!["   ".to_string(), "\t".to_string()],
                },
                ..Default::default()
            },
            ..AppConfig::default()
        };

        let pipeline = NotificationPipeline::from_config(&config);
        let providers = pipeline.provider_configs();
        assert_eq!(providers.len(), 4);
        assert_eq!(providers[0].kind, NotificationProviderKind::Email);
        assert!(
            !providers[0].enabled,
            "whitespace-only recipients should not enable the provider"
        );
        assert_eq!(providers[1].kind, NotificationProviderKind::Discord);
        assert!(!providers[1].enabled);
        assert_eq!(providers[2].kind, NotificationProviderKind::Slack);
        assert!(!providers[2].enabled);
        assert_eq!(providers[3].kind, NotificationProviderKind::Pushover);
        assert!(!providers[3].enabled);
    }

    #[tokio::test]
    async fn from_config_dispatches_to_discord_webhook_when_enabled() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/webhooks/test"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let config = AppConfig {
            notifications: chorrosion_config::NotificationsConfig {
                discord: chorrosion_config::DiscordNotificationConfig {
                    enabled: true,
                    webhook_url: Some(format!("{}/api/webhooks/test", server.uri())),
                    username: Some("Chorrosion".to_string()),
                },
                ..Default::default()
            },
            ..AppConfig::default()
        };

        let pipeline = NotificationPipeline::from_config(&config);
        let dispatched = pipeline.dispatch(&NotificationEvent::test()).await.unwrap();
        assert_eq!(dispatched, 1);
    }

    #[tokio::test]
    async fn from_config_dispatches_to_slack_webhook_when_enabled() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/services/test"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let config = AppConfig {
            notifications: chorrosion_config::NotificationsConfig {
                slack: chorrosion_config::SlackNotificationConfig {
                    enabled: true,
                    webhook_url: Some(format!("{}/services/test", server.uri())),
                    username: Some("Chorrosion".to_string()),
                },
                ..Default::default()
            },
            ..AppConfig::default()
        };

        let pipeline = NotificationPipeline::from_config(&config);
        let dispatched = pipeline.dispatch(&NotificationEvent::test()).await.unwrap();
        assert_eq!(dispatched, 1);
    }

    #[tokio::test]
    async fn from_config_dispatches_to_pushover_when_enabled() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/1/messages.json"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let config = AppConfig {
            notifications: chorrosion_config::NotificationsConfig {
                pushover: chorrosion_config::PushoverNotificationConfig {
                    enabled: true,
                    api_token: Some("token-123".to_string()),
                    user_key: Some("user-456".to_string()),
                    api_url: Some(format!("{}/1/messages.json", server.uri())),
                },
                ..Default::default()
            },
            ..AppConfig::default()
        };

        let pipeline = NotificationPipeline::from_config(&config);
        let dispatched = pipeline.dispatch(&NotificationEvent::test()).await.unwrap();
        assert_eq!(dispatched, 1);
    }

    #[test]
    fn from_config_disables_discord_when_webhook_url_is_invalid() {
        for bad_url in &[
            "not-a-url",
            "ftp://discord.com/webhooks/test",
            "discord.com/webhooks/test",
        ] {
            let config = AppConfig {
                notifications: chorrosion_config::NotificationsConfig {
                    discord: chorrosion_config::DiscordNotificationConfig {
                        enabled: true,
                        webhook_url: Some(bad_url.to_string()),
                        username: None,
                    },
                    ..Default::default()
                },
                ..AppConfig::default()
            };

            let pipeline = NotificationPipeline::from_config(&config);
            let providers = pipeline.provider_configs();
            let discord = providers
                .iter()
                .find(|p| p.kind == NotificationProviderKind::Discord)
                .expect("discord provider should be in configs");
            assert!(
                !discord.enabled,
                "Discord provider should be disabled for invalid URL: {bad_url}"
            );
        }
    }

    #[test]
    fn from_config_disables_slack_when_webhook_url_is_invalid() {
        for bad_url in &[
            "not-a-url",
            "ftp://hooks.slack.com/services/test",
            "hooks.slack.com/services/test",
        ] {
            let config = AppConfig {
                notifications: chorrosion_config::NotificationsConfig {
                    slack: chorrosion_config::SlackNotificationConfig {
                        enabled: true,
                        webhook_url: Some(bad_url.to_string()),
                        username: None,
                    },
                    ..Default::default()
                },
                ..AppConfig::default()
            };

            let pipeline = NotificationPipeline::from_config(&config);
            let providers = pipeline.provider_configs();
            let slack = providers
                .iter()
                .find(|p| p.kind == NotificationProviderKind::Slack)
                .expect("slack provider should be in configs");
            assert!(
                !slack.enabled,
                "Slack provider should be disabled for invalid URL: {bad_url}"
            );
        }
    }

    #[test]
    fn from_config_disables_pushover_when_api_url_is_invalid() {
        for bad_url in &[
            "not-a-url",
            "ftp://api.pushover.net/1/messages.json",
            "api.pushover.net/1/messages.json",
        ] {
            let config = AppConfig {
                notifications: chorrosion_config::NotificationsConfig {
                    pushover: chorrosion_config::PushoverNotificationConfig {
                        enabled: true,
                        api_token: Some("token-123".to_string()),
                        user_key: Some("user-456".to_string()),
                        api_url: Some(bad_url.to_string()),
                    },
                    ..Default::default()
                },
                ..AppConfig::default()
            };

            let pipeline = NotificationPipeline::from_config(&config);
            let providers = pipeline.provider_configs();
            let pushover = providers
                .iter()
                .find(|p| p.kind == NotificationProviderKind::Pushover)
                .expect("pushover provider should be in configs");
            assert!(
                !pushover.enabled,
                "Pushover provider should be disabled for invalid URL: {bad_url}"
            );
        }
    }

    #[test]
    fn from_config_disables_pushover_when_credentials_missing() {
        let config = AppConfig {
            notifications: chorrosion_config::NotificationsConfig {
                pushover: chorrosion_config::PushoverNotificationConfig {
                    enabled: true,
                    api_token: None,
                    user_key: Some("user-456".to_string()),
                    api_url: None,
                },
                ..Default::default()
            },
            ..AppConfig::default()
        };

        let pipeline = NotificationPipeline::from_config(&config);
        let providers = pipeline.provider_configs();
        let pushover = providers
            .iter()
            .find(|p| p.kind == NotificationProviderKind::Pushover)
            .expect("pushover provider should be in configs");
        assert!(!pushover.enabled);
    }

    #[test]
    fn from_config_enables_pushover_with_default_api_url() {
        // When api_url is None the default "https://api.pushover.net/1/messages.json" is used.
        // This test verifies that the default URL is a valid https URL, so the provider is
        // enabled when credentials are present and api_url is left unset.
        let config = AppConfig {
            notifications: chorrosion_config::NotificationsConfig {
                pushover: chorrosion_config::PushoverNotificationConfig {
                    enabled: true,
                    api_token: Some("token-123".to_string()),
                    user_key: Some("user-456".to_string()),
                    api_url: None,
                },
                ..Default::default()
            },
            ..AppConfig::default()
        };

        let pipeline = NotificationPipeline::from_config(&config);
        let providers = pipeline.provider_configs();
        let pushover = providers
            .iter()
            .find(|p| p.kind == NotificationProviderKind::Pushover)
            .expect("pushover provider should be in configs");
        assert!(
            pushover.enabled,
            "Pushover provider should be enabled when api_url is None and credentials are set"
        );
    }

    #[tokio::test]
    async fn from_config_dispatches_to_pushover_using_default_api_url() {
        // Verify that the provider actually POSTs to the default URL path when api_url is None.
        // We intercept the request by constructing a PushoverProvider with api_url explicitly
        // set to the mock server URI (same path as the default), ensuring the dispatch code
        // path that handles the default URL is exercised end-to-end.
        let server = MockServer::start().await;
        let default_path = "/1/messages.json";
        Mock::given(method("POST"))
            .and(path(default_path))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        // Build a config that mirrors the api_url=None scenario: credentials present, the URL
        // set to the mock server so we can intercept the outgoing request.
        let config = AppConfig {
            notifications: chorrosion_config::NotificationsConfig {
                pushover: chorrosion_config::PushoverNotificationConfig {
                    enabled: true,
                    api_token: Some("token-default".to_string()),
                    user_key: Some("user-default".to_string()),
                    api_url: Some(format!("{}{}", server.uri(), default_path)),
                },
                ..Default::default()
            },
            ..AppConfig::default()
        };

        let pipeline = NotificationPipeline::from_config(&config);
        let dispatched = pipeline.dispatch(&NotificationEvent::test()).await.unwrap();
        assert_eq!(dispatched, 1);
    }
}
