// SPDX-License-Identifier: GPL-3.0-or-later
use async_trait::async_trait;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadState {
    Queued,
    Downloading,
    Paused,
    Completed,
    Error,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadItem {
    pub hash: String,
    pub name: String,
    pub progress_percent: u8,
    pub category: Option<String>,
    pub state: DownloadState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTorrentRequest {
    pub torrent_or_magnet: String,
    pub category: Option<String>,
}

#[derive(Debug, Error)]
pub enum DownloadClientError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("authentication failed")]
    Authentication,
    #[error("invalid base url: {0}")]
    InvalidBaseUrl(String),
    #[error("download client responded with status {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("deserialization failed: {0}")]
    Deserialization(String),
}

#[async_trait]
pub trait DownloadClient: Send + Sync {
    async fn test_connection(&self) -> Result<(), DownloadClientError>;
    async fn add_torrent(&self, request: AddTorrentRequest) -> Result<(), DownloadClientError>;
    async fn set_category(&self, hash: &str, category: &str) -> Result<(), DownloadClientError>;
    async fn list_downloads(&self) -> Result<Vec<DownloadItem>, DownloadClientError>;
    async fn prioritize_download(&self, hash: &str) -> Result<(), DownloadClientError>;
}

fn build_download_client_http_client() -> Client {
    Client::builder()
        .user_agent(concat!(
            "chorrosion/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/SvetaKrava/chorrosion)"
        ))
        .timeout(Duration::from_secs(30))
        .cookie_store(true)
        .build()
        .unwrap_or_else(|error| {
            warn!(
                ?error,
                "Failed to build download client HTTP client with cookie store; session-based authentication may not work"
            );
            Client::new()
        })
}

pub struct QBittorrentClient {
    client: Client,
    base_url: String,
    username: Option<String>,
    password: Option<String>,
}

impl QBittorrentClient {
    pub fn new(base_url: String, username: Option<String>, password: Option<String>) -> Self {
        let client = build_download_client_http_client();
        let base_url = base_url.trim_end_matches('/').to_string();
        debug!(target: "download_clients", %base_url, "Initialized QBittorrentClient");
        Self {
            client,
            base_url,
            username,
            password,
        }
    }

    fn endpoint(&self, path: &str) -> Result<Url, DownloadClientError> {
        let base = Url::parse(&self.base_url)
            .map_err(|err| DownloadClientError::InvalidBaseUrl(err.to_string()))?;
        base.join(path)
            .map_err(|err| DownloadClientError::InvalidBaseUrl(err.to_string()))
    }

    async fn authenticate_if_configured(&self) -> Result<(), DownloadClientError> {
        let Some(username) = self.username.as_deref() else {
            return Ok(());
        };
        let Some(password) = self.password.as_deref() else {
            return Ok(());
        };

        let url = self.endpoint("/api/v2/auth/login")?;
        let response = self
            .client
            .post(url)
            .form(&[("username", username), ("password", password)])
            .send()
            .await
            .map_err(|e| DownloadClientError::Request(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| DownloadClientError::Request(e.to_string()))?;

        if !status.is_success() {
            return Err(DownloadClientError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        if body.trim() != "Ok." {
            return Err(DownloadClientError::Authentication);
        }

        Ok(())
    }

    async fn post_form(
        &self,
        path: &str,
        form: &HashMap<&str, String>,
    ) -> Result<(), DownloadClientError> {
        self.authenticate_if_configured().await?;
        let url = self.endpoint(path)?;

        let response = self
            .client
            .post(url)
            .form(form)
            .send()
            .await
            .map_err(|e| DownloadClientError::Request(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| DownloadClientError::Request(e.to_string()))?;

        if !status.is_success() {
            return Err(DownloadClientError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        Ok(())
    }
}

#[async_trait]
impl DownloadClient for QBittorrentClient {
    async fn test_connection(&self) -> Result<(), DownloadClientError> {
        self.authenticate_if_configured().await?;
        let url = self.endpoint("/api/v2/app/version")?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| DownloadClientError::Request(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DownloadClientError::HttpStatus {
                status: response.status().as_u16(),
                body: response
                    .text()
                    .await
                    .map_err(|e| DownloadClientError::Request(e.to_string()))?,
            });
        }

        Ok(())
    }

    async fn add_torrent(&self, request: AddTorrentRequest) -> Result<(), DownloadClientError> {
        let mut form = HashMap::new();
        form.insert("urls", request.torrent_or_magnet);
        if let Some(category) = request.category {
            form.insert("category", category);
        }

        self.post_form("/api/v2/torrents/add", &form).await
    }

    async fn set_category(&self, hash: &str, category: &str) -> Result<(), DownloadClientError> {
        let mut form = HashMap::new();
        form.insert("hashes", hash.to_string());
        form.insert("category", category.to_string());

        self.post_form("/api/v2/torrents/setCategory", &form).await
    }

    async fn list_downloads(&self) -> Result<Vec<DownloadItem>, DownloadClientError> {
        self.authenticate_if_configured().await?;
        let url = self.endpoint("/api/v2/torrents/info")?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| DownloadClientError::Request(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| DownloadClientError::Request(e.to_string()))?;

        if !status.is_success() {
            return Err(DownloadClientError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        let torrents: Vec<QBittorrentTorrent> = serde_json::from_str(&body)
            .map_err(|e| DownloadClientError::Deserialization(e.to_string()))?;

        Ok(torrents
            .into_iter()
            .map(|torrent| DownloadItem {
                hash: torrent.hash,
                name: torrent.name,
                progress_percent: (torrent.progress * 100.0).round().clamp(0.0, 100.0) as u8,
                category: torrent
                    .category
                    .filter(|v| !v.trim().is_empty()),
                state: map_qbittorrent_state(&torrent.state),
            })
            .collect())
    }

    async fn prioritize_download(&self, hash: &str) -> Result<(), DownloadClientError> {
        let mut form = HashMap::new();
        form.insert("hashes", hash.to_string());

        self.post_form("/api/v2/torrents/topPrio", &form).await
    }
}

#[derive(Debug, Deserialize)]
struct QBittorrentTorrent {
    hash: String,
    name: String,
    #[serde(default)]
    progress: f32,
    #[serde(default)]
    state: String,
    #[serde(default)]
    category: Option<String>,
}

fn map_qbittorrent_state(state: &str) -> DownloadState {
    let state = state.to_lowercase();
    if state.contains("error") || state.contains("missingfiles") {
        DownloadState::Error
    } else if state.contains("paused") || state.contains("stalled") {
        DownloadState::Paused
    } else if state.contains("uploading") || state.contains("completed") {
        DownloadState::Completed
    } else if state.contains("downloading") || state.contains("meta") || state.contains("forceddl") {
        DownloadState::Downloading
    } else if state.contains("queued") {
        DownloadState::Queued
    } else {
        DownloadState::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::{AddTorrentRequest, DownloadClient, DownloadState, QBittorrentClient};
    use wiremock::matchers::{body_string_contains, method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_connection_success() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v2/app/version"))
            .respond_with(ResponseTemplate::new(200).set_body_string("4.6.7"))
            .mount(&server)
            .await;

        let client = QBittorrentClient::new(server.uri(), None, None);
        let result = client.test_connection().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn add_torrent_posts_to_qbittorrent() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path_regex("/api/v2/torrents/add|/api/v2/torrents/add/"))
            .and(body_string_contains("urls=magnet%3A%3Fxt%3Durn%3Abtih%3Atest"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = QBittorrentClient::new(server.uri(), None, None);
        let result = client
            .add_torrent(AddTorrentRequest {
                torrent_or_magnet: "magnet:?xt=urn:btih:test".to_string(),
                category: Some("music".to_string()),
            })
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn list_downloads_maps_state_and_progress() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v2/torrents/info"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[
                    {
                        "hash": "abc123",
                        "name": "Album FLAC",
                        "progress": 0.53,
                        "state": "downloading",
                        "category": "music"
                    }
                ]"#,
            ))
            .mount(&server)
            .await;

        let client = QBittorrentClient::new(server.uri(), None, None);
        let downloads = client
            .list_downloads()
            .await
            .expect("downloads should parse");

        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].hash, "abc123");
        assert_eq!(downloads[0].progress_percent, 53);
        assert_eq!(downloads[0].state, DownloadState::Downloading);
        assert_eq!(downloads[0].category.as_deref(), Some("music"));
    }

    #[tokio::test]
    async fn prioritize_download_posts_hash() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v2/torrents/topPrio"))
            .and(body_string_contains("hashes=abc123"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = QBittorrentClient::new(server.uri(), None, None);
        let result = client.prioritize_download("abc123").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn set_category_posts_hash_and_category() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v2/torrents/setCategory"))
            .and(body_string_contains("hashes=abc123"))
            .and(body_string_contains("category=music"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = QBittorrentClient::new(server.uri(), None, None);
        let result = client.set_category("abc123", "music").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn authentication_succeeds_with_valid_credentials() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v2/auth/login"))
            .and(body_string_contains("username=admin"))
            .and(body_string_contains("password=secret"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Ok."))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/v2/app/version"))
            .respond_with(ResponseTemplate::new(200).set_body_string("4.6.7"))
            .mount(&server)
            .await;

        let client = QBittorrentClient::new(
            server.uri(),
            Some("admin".to_string()),
            Some("secret".to_string()),
        );
        let result = client.test_connection().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn authentication_fails_with_wrong_credentials() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v2/auth/login"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Fails."))
            .mount(&server)
            .await;

        let client = QBittorrentClient::new(
            server.uri(),
            Some("admin".to_string()),
            Some("wrong".to_string()),
        );
        let result = client.test_connection().await;

        assert!(matches!(
            result,
            Err(super::DownloadClientError::Authentication)
        ));
    }

    #[tokio::test]
    async fn authentication_fails_on_http_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v2/auth/login"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
            .mount(&server)
            .await;

        let client = QBittorrentClient::new(
            server.uri(),
            Some("admin".to_string()),
            Some("secret".to_string()),
        );
        let result = client.test_connection().await;

        assert!(matches!(
            result,
            Err(super::DownloadClientError::HttpStatus { status: 403, .. })
        ));
    }

    #[test]
    fn state_mapping_error_states() {
        use super::map_qbittorrent_state;

        assert_eq!(map_qbittorrent_state("error"), DownloadState::Error);
        assert_eq!(
            map_qbittorrent_state("missingFiles"),
            DownloadState::Error
        );
    }

    #[test]
    fn state_mapping_paused_states() {
        use super::map_qbittorrent_state;

        assert_eq!(map_qbittorrent_state("pausedDL"), DownloadState::Paused);
        assert_eq!(map_qbittorrent_state("stalledDL"), DownloadState::Paused);
        assert_eq!(map_qbittorrent_state("pausedUP"), DownloadState::Paused);
    }

    #[test]
    fn state_mapping_completed_states() {
        use super::map_qbittorrent_state;

        assert_eq!(map_qbittorrent_state("uploading"), DownloadState::Completed);
    }

    #[test]
    fn state_mapping_downloading_states() {
        use super::map_qbittorrent_state;

        assert_eq!(
            map_qbittorrent_state("downloading"),
            DownloadState::Downloading
        );
        assert_eq!(
            map_qbittorrent_state("forcedDL"),
            DownloadState::Downloading
        );
    }

    #[test]
    fn state_mapping_queued_states() {
        use super::map_qbittorrent_state;

        assert_eq!(map_qbittorrent_state("queuedDL"), DownloadState::Queued);
        assert_eq!(map_qbittorrent_state("queuedUP"), DownloadState::Queued);
    }

    #[test]
    fn state_mapping_unknown_state() {
        use super::map_qbittorrent_state;

        assert_eq!(
            map_qbittorrent_state("forcedUP"),
            DownloadState::Unknown
        );
        assert_eq!(
            map_qbittorrent_state("checkingUP"),
            DownloadState::Unknown
        );
        assert_eq!(
            map_qbittorrent_state("something_unexpected"),
            DownloadState::Unknown
        );
    }
}
