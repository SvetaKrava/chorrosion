// SPDX-License-Identifier: GPL-3.0-or-later
use async_trait::async_trait;
use reqwest::{Client, Url};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
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

pub struct TransmissionClient {
    client: Client,
    base_url: String,
    username: Option<String>,
    password: Option<String>,
    session_id: RwLock<Option<String>>,
}

pub struct DelugeClient {
    client: Client,
    base_url: String,
    password: Option<String>,
}

pub struct SabnzbdClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl DelugeClient {
    pub fn new(base_url: String, password: Option<String>) -> Self {
        let client = build_download_client_http_client();
        let base_url = base_url.trim_end_matches('/').to_string();
        debug!(target: "download_clients", %base_url, "Initialized DelugeClient");
        Self {
            client,
            base_url,
            password,
        }
    }

    fn endpoint(&self) -> Result<Url, DownloadClientError> {
        let mut base = Url::parse(&self.base_url)
            .map_err(|err| DownloadClientError::InvalidBaseUrl(err.to_string()))?;
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        base.join("json")
            .map_err(|err| DownloadClientError::InvalidBaseUrl(err.to_string()))
    }

    async fn rpc_call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, DownloadClientError> {
        let url = self.endpoint()?;
        let payload = json!({
            "method": method,
            "params": params,
            "id": 1,
        });

        let response = self
            .client
            .post(url)
            .json(&payload)
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

        let rpc: DelugeRpcResponse<T> = serde_json::from_str(&body)
            .map_err(|e| DownloadClientError::Deserialization(e.to_string()))?;
        if let Some(error) = rpc.error {
            return Err(DownloadClientError::Request(format!(
                "deluge RPC error: {}",
                error.message
            )));
        }
        Ok(rpc.result)
    }

    async fn authenticate_if_configured(&self) -> Result<(), DownloadClientError> {
        let Some(password) = self.password.as_deref() else {
            return Ok(());
        };

        let logged_in: bool = self.rpc_call("auth.login", json!([password])).await?;
        if logged_in {
            Ok(())
        } else {
            Err(DownloadClientError::Authentication)
        }
    }
}

impl SabnzbdClient {
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        let client = build_download_client_http_client();
        let base_url = base_url.trim_end_matches('/').to_string();
        debug!(target: "download_clients", %base_url, "Initialized SabnzbdClient");
        Self {
            client,
            base_url,
            api_key,
        }
    }

    fn endpoint(&self) -> Result<Url, DownloadClientError> {
        let mut base = Url::parse(&self.base_url)
            .map_err(|err| DownloadClientError::InvalidBaseUrl(err.to_string()))?;
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        base.join("api")
            .map_err(|err| DownloadClientError::InvalidBaseUrl(err.to_string()))
    }

    async fn api_get(&self, mut params: Vec<(&str, String)>) -> Result<Value, DownloadClientError> {
        let url = self.endpoint()?;
        params.push(("output", "json".to_string()));
        if let Some(api_key) = self.api_key.clone() {
            params.push(("apikey", api_key));
        }

        let response = self
            .client
            .get(url)
            .query(&params)
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

        serde_json::from_str(&body).map_err(|e| DownloadClientError::Deserialization(e.to_string()))
    }
}

impl TransmissionClient {
    pub fn new(base_url: String, username: Option<String>, password: Option<String>) -> Self {
        let client = build_download_client_http_client();
        let base_url = base_url.trim_end_matches('/').to_string();
        debug!(target: "download_clients", %base_url, "Initialized TransmissionClient");
        Self {
            client,
            base_url,
            username,
            password,
            session_id: RwLock::new(None),
        }
    }

    fn endpoint(&self) -> Result<Url, DownloadClientError> {
        let base = Url::parse(&self.base_url)
            .map_err(|err| DownloadClientError::InvalidBaseUrl(err.to_string()))?;
        base.join("/transmission/rpc")
            .map_err(|err| DownloadClientError::InvalidBaseUrl(err.to_string()))
    }

    async fn rpc_call<T: DeserializeOwned>(
        &self,
        method: &str,
        arguments: Value,
    ) -> Result<T, DownloadClientError> {
        let url = self.endpoint()?;
        let payload = json!({
            "method": method,
            "arguments": arguments,
        });

        let mut request = self.client.post(url.clone()).json(&payload);
        if let Some(username) = self.username.as_deref() {
            request = request.basic_auth(username, self.password.as_deref());
        }
        if let Some(session_id) = self.session_id.read().await.clone() {
            request = request.header("X-Transmission-Session-Id", session_id);
        }

        let mut response = request
            .send()
            .await
            .map_err(|e| DownloadClientError::Request(e.to_string()))?;

        if response.status() == reqwest::StatusCode::CONFLICT {
            if let Some(session_id) = response
                .headers()
                .get("X-Transmission-Session-Id")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
            {
                *self.session_id.write().await = Some(session_id.clone());
                let mut retry = self
                    .client
                    .post(url)
                    .header("X-Transmission-Session-Id", session_id)
                    .json(&payload);
                if let Some(username) = self.username.as_deref() {
                    retry = retry.basic_auth(username, self.password.as_deref());
                }
                response = retry
                    .send()
                    .await
                    .map_err(|e| DownloadClientError::Request(e.to_string()))?;
            }
        }

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

        let rpc: TransmissionRpcResponse<T> = serde_json::from_str(&body)
            .map_err(|e| DownloadClientError::Deserialization(e.to_string()))?;
        if rpc.result != "success" {
            return Err(DownloadClientError::Request(format!(
                "transmission RPC error: {}",
                rpc.result
            )));
        }

        Ok(rpc.arguments)
    }
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
                category: torrent.category.filter(|v| !v.trim().is_empty()),
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

#[async_trait]
impl DownloadClient for TransmissionClient {
    async fn test_connection(&self) -> Result<(), DownloadClientError> {
        let _: Value = self.rpc_call("session-get", json!({})).await?;
        Ok(())
    }

    async fn add_torrent(&self, request: AddTorrentRequest) -> Result<(), DownloadClientError> {
        let mut args = json!({
            "filename": request.torrent_or_magnet,
        });
        if let Some(category) = request.category {
            args["download-dir"] = json!(category);
        }
        let _: Value = self.rpc_call("torrent-add", args).await?;
        Ok(())
    }

    async fn set_category(&self, hash: &str, category: &str) -> Result<(), DownloadClientError> {
        let _: Value = self
            .rpc_call(
                "torrent-set-location",
                json!({
                    "ids": [hash],
                    "location": category,
                    "move": false
                }),
            )
            .await?;
        Ok(())
    }

    async fn list_downloads(&self) -> Result<Vec<DownloadItem>, DownloadClientError> {
        let torrents: TransmissionTorrentGetArguments = self
            .rpc_call(
                "torrent-get",
                json!({
                    "fields": ["hashString", "name", "percentDone", "status", "downloadDir"]
                }),
            )
            .await?;

        Ok(torrents
            .torrents
            .into_iter()
            .map(|torrent| DownloadItem {
                hash: torrent.hash_string,
                name: torrent.name,
                progress_percent: (torrent.percent_done * 100.0).round().clamp(0.0, 100.0) as u8,
                category: torrent.download_dir.filter(|v| !v.trim().is_empty()),
                state: map_transmission_state(torrent.status),
            })
            .collect())
    }

    async fn prioritize_download(&self, hash: &str) -> Result<(), DownloadClientError> {
        let _: Value = self
            .rpc_call("queue-move-top", json!({ "ids": [hash] }))
            .await?;
        Ok(())
    }
}

#[async_trait]
impl DownloadClient for DelugeClient {
    async fn test_connection(&self) -> Result<(), DownloadClientError> {
        self.authenticate_if_configured().await?;
        let _: Value = self.rpc_call("web.connected", json!([])).await?;
        Ok(())
    }

    async fn add_torrent(&self, request: AddTorrentRequest) -> Result<(), DownloadClientError> {
        self.authenticate_if_configured().await?;
        let options = if let Some(category) = request.category {
            json!({ "download_location": category })
        } else {
            json!({})
        };

        let _: Value = self
            .rpc_call(
                "web.add_torrents",
                json!([[{
                    "path": request.torrent_or_magnet,
                    "options": options
                }]]),
            )
            .await?;
        Ok(())
    }

    async fn set_category(&self, hash: &str, category: &str) -> Result<(), DownloadClientError> {
        self.authenticate_if_configured().await?;
        let _: Value = self
            .rpc_call(
                "core.set_torrent_options",
                json!([[hash], { "download_location": category }]),
            )
            .await?;
        Ok(())
    }

    async fn list_downloads(&self) -> Result<Vec<DownloadItem>, DownloadClientError> {
        self.authenticate_if_configured().await?;
        let torrents: HashMap<String, DelugeTorrent> = self
            .rpc_call(
                "web.get_torrents_status",
                json!([
                    {},
                    ["name", "progress", "state", "label", "download_location"]
                ]),
            )
            .await?;

        Ok(torrents
            .into_iter()
            .map(|(hash, torrent)| {
                let category = torrent
                    .download_location
                    .or(torrent.label)
                    .filter(|v| !v.trim().is_empty());
                DownloadItem {
                    hash,
                    name: torrent.name,
                    progress_percent: torrent.progress.round().clamp(0.0, 100.0) as u8,
                    category,
                    state: map_deluge_state(&torrent.state),
                }
            })
            .collect())
    }

    async fn prioritize_download(&self, hash: &str) -> Result<(), DownloadClientError> {
        self.authenticate_if_configured().await?;
        let _: Value = self.rpc_call("core.queue_top", json!([[hash]])).await?;
        Ok(())
    }
}

#[async_trait]
impl DownloadClient for SabnzbdClient {
    async fn test_connection(&self) -> Result<(), DownloadClientError> {
        let response = self.api_get(vec![("mode", "version".to_string())]).await?;
        let version = response
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if version.is_empty() {
            return Err(DownloadClientError::Request(
                "sabnzbd version endpoint did not return a version".to_string(),
            ));
        }
        Ok(())
    }

    async fn add_torrent(&self, request: AddTorrentRequest) -> Result<(), DownloadClientError> {
        let mut params = vec![
            ("mode", "addurl".to_string()),
            ("name", request.torrent_or_magnet),
        ];
        if let Some(category) = request.category {
            params.push(("cat", category));
        }
        let response = self.api_get(params).await?;
        if !response
            .get("status")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(DownloadClientError::Request(
                "sabnzbd failed to add URL".to_string(),
            ));
        }
        Ok(())
    }

    async fn set_category(&self, hash: &str, category: &str) -> Result<(), DownloadClientError> {
        let response = self
            .api_get(vec![
                ("mode", "change_cat".to_string()),
                ("name", hash.to_string()),
                ("value", category.to_string()),
            ])
            .await?;
        if !response
            .get("status")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(DownloadClientError::Request(
                "sabnzbd failed to change category".to_string(),
            ));
        }
        Ok(())
    }

    async fn list_downloads(&self) -> Result<Vec<DownloadItem>, DownloadClientError> {
        let response = self
            .api_get(vec![
                ("mode", "queue".to_string()),
                ("start", "0".to_string()),
                ("limit", "200".to_string()),
            ])
            .await?;
        let queue: SabnzbdQueueResponse = serde_json::from_value(response)
            .map_err(|e| DownloadClientError::Deserialization(e.to_string()))?;

        Ok(queue
            .queue
            .slots
            .into_iter()
            .map(|slot| DownloadItem {
                hash: slot.nzo_id,
                name: slot.filename,
                progress_percent: slot
                    .percentage
                    .parse::<f32>()
                    .ok()
                    .map(|v| v.round().clamp(0.0, 100.0) as u8)
                    .unwrap_or(0),
                category: slot.cat.filter(|v| !v.trim().is_empty()),
                state: map_sabnzbd_state(slot.status.as_deref().or(queue.queue.status.as_deref())),
            })
            .collect())
    }

    async fn prioritize_download(&self, hash: &str) -> Result<(), DownloadClientError> {
        let response = self
            .api_get(vec![
                ("mode", "queue".to_string()),
                ("name", "priority".to_string()),
                ("value", "2".to_string()),
                ("nzo_ids", hash.to_string()),
            ])
            .await?;
        if !response
            .get("status")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(DownloadClientError::Request(
                "sabnzbd failed to update priority".to_string(),
            ));
        }
        Ok(())
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

#[derive(Debug, Deserialize)]
struct TransmissionRpcResponse<T> {
    result: String,
    arguments: T,
}

#[derive(Debug, Deserialize)]
struct TransmissionTorrentGetArguments {
    torrents: Vec<TransmissionTorrent>,
}

#[derive(Debug, Deserialize)]
struct TransmissionTorrent {
    #[serde(rename = "hashString")]
    hash_string: String,
    name: String,
    #[serde(default, rename = "percentDone")]
    percent_done: f32,
    #[serde(default)]
    status: i64,
    #[serde(default, rename = "downloadDir")]
    download_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DelugeRpcResponse<T> {
    result: T,
    error: Option<DelugeRpcError>,
}

#[derive(Debug, Deserialize)]
struct DelugeRpcError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct DelugeTorrent {
    name: String,
    #[serde(default)]
    progress: f32,
    #[serde(default)]
    state: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default, rename = "download_location")]
    download_location: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SabnzbdQueueResponse {
    queue: SabnzbdQueue,
}

#[derive(Debug, Deserialize)]
struct SabnzbdQueue {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    slots: Vec<SabnzbdQueueSlot>,
}

#[derive(Debug, Deserialize)]
struct SabnzbdQueueSlot {
    nzo_id: String,
    filename: String,
    #[serde(default)]
    percentage: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    cat: Option<String>,
}

fn map_qbittorrent_state(state: &str) -> DownloadState {
    let state = state.to_lowercase();
    if state.contains("error") || state.contains("missingfiles") {
        DownloadState::Error
    } else if state.contains("paused") || state.contains("stalled") {
        DownloadState::Paused
    } else if state.contains("uploading") || state.contains("completed") {
        DownloadState::Completed
    } else if state.contains("downloading") || state.contains("meta") || state.contains("forceddl")
    {
        DownloadState::Downloading
    } else if state.contains("queued") {
        DownloadState::Queued
    } else {
        DownloadState::Unknown
    }
}

fn map_transmission_state(status: i64) -> DownloadState {
    match status {
        0 => DownloadState::Paused,
        1..=3 => DownloadState::Queued,
        4 => DownloadState::Downloading,
        5 | 6 => DownloadState::Completed,
        _ => DownloadState::Unknown,
    }
}

fn map_deluge_state(state: &str) -> DownloadState {
    match state.to_lowercase().as_str() {
        s if s.contains("error") => DownloadState::Error,
        s if s.contains("paused") => DownloadState::Paused,
        s if s.contains("queued") => DownloadState::Queued,
        s if s.contains("seeding") || s.contains("finished") => DownloadState::Completed,
        s if s.contains("downloading") || s.contains("checking") => DownloadState::Downloading,
        _ => DownloadState::Unknown,
    }
}

fn map_sabnzbd_state(state: Option<&str>) -> DownloadState {
    match state.unwrap_or_default().to_lowercase().as_str() {
        s if s.contains("failed") || s.contains("error") => DownloadState::Error,
        s if s.contains("paused") => DownloadState::Paused,
        s if s.contains("queued") || s.contains("fetching") => DownloadState::Queued,
        s if s.contains("completed") || s.contains("idle") => DownloadState::Completed,
        s if s.contains("downloading") || s.contains("extracting") => DownloadState::Downloading,
        _ => DownloadState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_deluge_state, map_sabnzbd_state, map_transmission_state, AddTorrentRequest,
        DelugeClient, DownloadClient, DownloadState, QBittorrentClient, SabnzbdClient,
        TransmissionClient,
    };
    use wiremock::matchers::{body_string_contains, method, path, path_regex, query_param};
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
            .and(body_string_contains(
                "urls=magnet%3A%3Fxt%3Durn%3Abtih%3Atest",
            ))
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
        assert_eq!(map_qbittorrent_state("missingFiles"), DownloadState::Error);
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

        assert_eq!(map_qbittorrent_state("forcedUP"), DownloadState::Unknown);
        assert_eq!(map_qbittorrent_state("checkingUP"), DownloadState::Unknown);
        assert_eq!(
            map_qbittorrent_state("something_unexpected"),
            DownloadState::Unknown
        );
    }

    #[tokio::test]
    async fn transmission_test_connection_negotiates_session_and_succeeds() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/transmission/rpc"))
            .and(wiremock::matchers::header(
                "X-Transmission-Session-Id",
                "session-1",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"result":"success","arguments":{}}"#),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/transmission/rpc"))
            .respond_with(
                ResponseTemplate::new(409).insert_header("X-Transmission-Session-Id", "session-1"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = TransmissionClient::new(server.uri(), None, None);
        let result = client.test_connection().await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn transmission_add_torrent_posts_filename() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/transmission/rpc"))
            .and(body_string_contains("\"method\":\"torrent-add\""))
            .and(body_string_contains(
                "\"filename\":\"magnet:?xt=urn:btih:test\"",
            ))
            .and(body_string_contains(
                "\"download-dir\":\"/downloads/music\"",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"result":"success","arguments":{}}"#),
            )
            .mount(&server)
            .await;

        let client = TransmissionClient::new(server.uri(), None, None);
        let result = client
            .add_torrent(AddTorrentRequest {
                torrent_or_magnet: "magnet:?xt=urn:btih:test".to_string(),
                category: Some("/downloads/music".to_string()),
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn transmission_set_category_posts_torrent_set_location() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/transmission/rpc"))
            .and(body_string_contains("\"method\":\"torrent-set-location\""))
            .and(body_string_contains("\"ids\":[\"abc123\"]"))
            .and(body_string_contains("\"location\":\"/downloads/music\""))
            .and(body_string_contains("\"move\":false"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"result":"success","arguments":{}}"#),
            )
            .mount(&server)
            .await;

        let client = TransmissionClient::new(server.uri(), None, None);
        let result = client.set_category("abc123", "/downloads/music").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn transmission_list_downloads_maps_state_and_progress() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/transmission/rpc"))
            .and(body_string_contains("\"method\":\"torrent-get\""))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{
                    "result":"success",
                    "arguments":{
                        "torrents":[
                            {
                                "hashString":"abc123",
                                "name":"Album FLAC",
                                "percentDone":0.42,
                                "status":4,
                                "downloadDir":"/downloads/music"
                            }
                        ]
                    }
                }"#,
            ))
            .mount(&server)
            .await;

        let client = TransmissionClient::new(server.uri(), None, None);
        let downloads = client.list_downloads().await.expect("downloads parse");
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].hash, "abc123");
        assert_eq!(downloads[0].progress_percent, 42);
        assert_eq!(downloads[0].state, DownloadState::Downloading);
    }

    #[tokio::test]
    async fn transmission_prioritize_download_posts_queue_move_top() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/transmission/rpc"))
            .and(body_string_contains("\"method\":\"queue-move-top\""))
            .and(body_string_contains("abc123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"result":"success","arguments":{}}"#),
            )
            .mount(&server)
            .await;

        let client = TransmissionClient::new(server.uri(), None, None);
        let result = client.prioritize_download("abc123").await;
        assert!(result.is_ok());
    }

    #[test]
    fn transmission_state_mapping() {
        assert_eq!(map_transmission_state(0), DownloadState::Paused);
        assert_eq!(map_transmission_state(3), DownloadState::Queued);
        assert_eq!(map_transmission_state(4), DownloadState::Downloading);
        assert_eq!(map_transmission_state(6), DownloadState::Completed);
        assert_eq!(map_transmission_state(42), DownloadState::Unknown);
    }

    #[tokio::test]
    async fn deluge_test_connection_authenticates_and_checks_connected() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/json"))
            .and(body_string_contains("\"method\":\"auth.login\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"result":true,"error":null,"id":1}"#),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/json"))
            .and(body_string_contains("\"method\":\"web.connected\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"result":true,"error":null,"id":1}"#),
            )
            .mount(&server)
            .await;

        let client = DelugeClient::new(server.uri(), Some("secret".to_string()));
        let result = client.test_connection().await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn deluge_add_torrent_posts_web_add_torrents() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/json"))
            .and(body_string_contains("\"method\":\"web.add_torrents\""))
            .and(body_string_contains("magnet:?xt=urn:btih:test"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"result":{},"error":null,"id":1}"#),
            )
            .mount(&server)
            .await;

        let client = DelugeClient::new(server.uri(), None);
        let result = client
            .add_torrent(AddTorrentRequest {
                torrent_or_magnet: "magnet:?xt=urn:btih:test".to_string(),
                category: Some("/downloads/music".to_string()),
            })
            .await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn deluge_list_downloads_maps_state_and_progress() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/json"))
            .and(body_string_contains(
                "\"method\":\"web.get_torrents_status\"",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{
                    "result":{
                        "abc123":{
                            "name":"Album FLAC",
                            "progress":55.2,
                            "state":"Downloading",
                            "label":"music",
                            "download_location":"/downloads/music"
                        }
                    },
                    "error":null,
                    "id":1
                }"#,
            ))
            .mount(&server)
            .await;

        let client = DelugeClient::new(server.uri(), None);
        let downloads = client.list_downloads().await.expect("downloads parse");
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].hash, "abc123");
        assert_eq!(downloads[0].progress_percent, 55);
        assert_eq!(downloads[0].state, DownloadState::Downloading);
        assert_eq!(downloads[0].category.as_deref(), Some("/downloads/music"));
    }

    #[tokio::test]
    async fn deluge_prioritize_download_posts_queue_top() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/json"))
            .and(body_string_contains("\"method\":\"core.queue_top\""))
            .and(body_string_contains("abc123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"result":null,"error":null,"id":1}"#),
            )
            .mount(&server)
            .await;

        let client = DelugeClient::new(server.uri(), None);
        let result = client.prioritize_download("abc123").await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn deluge_set_category_posts_set_torrent_options() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/json"))
            .and(body_string_contains(
                "\"method\":\"core.set_torrent_options\"",
            ))
            .and(body_string_contains("abc123"))
            .and(body_string_contains("/downloads/lossless"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"result":true,"error":null,"id":1}"#),
            )
            .mount(&server)
            .await;

        let client = DelugeClient::new(server.uri(), None);
        let result = client.set_category("abc123", "/downloads/lossless").await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn deluge_state_mapping() {
        assert_eq!(map_deluge_state("Error"), DownloadState::Error);
        assert_eq!(map_deluge_state("Paused"), DownloadState::Paused);
        assert_eq!(map_deluge_state("Queued"), DownloadState::Queued);
        assert_eq!(map_deluge_state("Seeding"), DownloadState::Completed);
        assert_eq!(map_deluge_state("Downloading"), DownloadState::Downloading);
        assert_eq!(map_deluge_state("UnknownState"), DownloadState::Unknown);
    }

    #[tokio::test]
    async fn sabnzbd_test_connection_calls_version_api() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api"))
            .and(query_param("mode", "version"))
            .and(query_param("output", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"version":"4.3.0"}"#))
            .mount(&server)
            .await;

        let client = SabnzbdClient::new(server.uri(), None);
        let result = client.test_connection().await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn sabnzbd_add_torrent_calls_addurl_api() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api"))
            .and(query_param("mode", "addurl"))
            .and(query_param("name", "https://example.com/release.nzb"))
            .and(query_param("cat", "music"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":true}"#))
            .mount(&server)
            .await;

        let client = SabnzbdClient::new(server.uri(), None);
        let result = client
            .add_torrent(AddTorrentRequest {
                torrent_or_magnet: "https://example.com/release.nzb".to_string(),
                category: Some("music".to_string()),
            })
            .await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn sabnzbd_set_category_calls_change_cat_api() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api"))
            .and(query_param("mode", "change_cat"))
            .and(query_param("name", "SAB123"))
            .and(query_param("value", "lossless"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":true}"#))
            .mount(&server)
            .await;

        let client = SabnzbdClient::new(server.uri(), None);
        let result = client.set_category("SAB123", "lossless").await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn sabnzbd_list_downloads_maps_state_and_progress() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api"))
            .and(query_param("mode", "queue"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{
                    "queue": {
                        "status": "Downloading",
                        "slots": [
                            {
                                "nzo_id": "SAB123",
                                "filename": "Album FLAC.nzb",
                                "percentage": "37.4",
                                "cat": "music"
                            }
                        ]
                    }
                }"#,
            ))
            .mount(&server)
            .await;

        let client = SabnzbdClient::new(server.uri(), None);
        let downloads = client.list_downloads().await.expect("downloads parse");
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].hash, "SAB123");
        assert_eq!(downloads[0].name, "Album FLAC.nzb");
        assert_eq!(downloads[0].progress_percent, 37);
        assert_eq!(downloads[0].state, DownloadState::Downloading);
        assert_eq!(downloads[0].category.as_deref(), Some("music"));
    }

    #[tokio::test]
    async fn sabnzbd_prioritize_download_calls_queue_priority_api() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api"))
            .and(query_param("mode", "queue"))
            .and(query_param("name", "priority"))
            .and(query_param("nzo_ids", "SAB123"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":true}"#))
            .mount(&server)
            .await;

        let client = SabnzbdClient::new(server.uri(), None);
        let result = client.prioritize_download("SAB123").await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn sabnzbd_state_mapping() {
        assert_eq!(
            map_sabnzbd_state(Some("Downloading")),
            DownloadState::Downloading
        );
        assert_eq!(map_sabnzbd_state(Some("Paused")), DownloadState::Paused);
        assert_eq!(map_sabnzbd_state(Some("Queued")), DownloadState::Queued);
        assert_eq!(
            map_sabnzbd_state(Some("Completed")),
            DownloadState::Completed
        );
        assert_eq!(map_sabnzbd_state(Some("Failed")), DownloadState::Error);
        assert_eq!(map_sabnzbd_state(None), DownloadState::Unknown);
    }
}
