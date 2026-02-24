use moka::sync::Cache;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::{self, Value};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::{debug, instrument, warn};

use crate::fanarttv::FanartTvClient;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverArtProvider {
    FanartTv,
    CoverArtArchive,
}

impl CoverArtProvider {
    fn as_str(&self) -> &'static str {
        match self {
            Self::FanartTv => "fanarttv",
            Self::CoverArtArchive => "coverartarchive",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverArtResult {
    pub image_url: String,
    pub provider: CoverArtProvider,
}

pub struct CoverArtFallbackClient {
    fanart_client: Option<FanartTvClient>,
    cover_art_archive_client: CoverArtArchiveClient,
    provider_order: Vec<CoverArtProvider>,
    rate_limiter: Arc<Semaphore>,
    cache: Cache<String, CoverArtResult>,
}

impl CoverArtFallbackClient {
    pub fn new(
        fanart_client: Option<FanartTvClient>,
        cover_art_archive_base_url: Option<String>,
    ) -> Self {
        Self::new_with_order_and_limits(
            fanart_client,
            cover_art_archive_base_url,
            vec![CoverArtProvider::FanartTv, CoverArtProvider::CoverArtArchive],
            1,
        )
    }

    pub fn new_with_order_and_limits(
        fanart_client: Option<FanartTvClient>,
        cover_art_archive_base_url: Option<String>,
        provider_order: Vec<CoverArtProvider>,
        max_concurrent_requests: usize,
    ) -> Self {
        Self {
            fanart_client,
            cover_art_archive_client: CoverArtArchiveClient::new(cover_art_archive_base_url),
            provider_order,
            rate_limiter: Arc::new(Semaphore::new(max_concurrent_requests.max(1))),
            cache: Cache::new(10_000),
        }
    }

    #[instrument(skip(self), fields(release_group_mbid = release_group_mbid))]
    pub async fn fetch_album_cover(
        &self,
        release_group_mbid: &str,
    ) -> Result<CoverArtResult, CoverArtFallbackError> {
        if let Some(cached) = self.cache.get(release_group_mbid) {
            return Ok(cached);
        }

        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|_| CoverArtFallbackError::RateLimiterClosed)?;

        let mut provider_errors = Vec::new();
        let mut provider_attempts = 0usize;

        for provider in &self.provider_order {
            match provider {
                CoverArtProvider::FanartTv => {
                    let Some(client) = self.fanart_client.as_ref() else {
                        continue;
                    };

                    provider_attempts += 1;
                    match client.fetch_album_artwork(release_group_mbid).await {
                        Ok(artwork) => {
                            if let Some(image) = artwork.covers.first() {
                                let result = CoverArtResult {
                                    image_url: image.url.clone(),
                                    provider: CoverArtProvider::FanartTv,
                                };
                                self.cache.insert(release_group_mbid.to_string(), result.clone());
                                return Ok(result);
                            }
                            debug!(target: "cover-art", provider = provider.as_str(), "no cover returned from provider");
                        }
                        Err(error) => {
                            warn!(target: "cover-art", provider = provider.as_str(), error = %error, "provider failed");
                            provider_errors.push(ProviderError {
                                provider: CoverArtProvider::FanartTv,
                                message: error.to_string(),
                            });
                        }
                    }
                }
                CoverArtProvider::CoverArtArchive => {
                    provider_attempts += 1;
                    match self
                        .cover_art_archive_client
                        .fetch_album_cover(release_group_mbid)
                        .await
                    {
                        Ok(Some(image_url)) => {
                            let result = CoverArtResult {
                                image_url,
                                provider: CoverArtProvider::CoverArtArchive,
                            };
                            self.cache.insert(release_group_mbid.to_string(), result.clone());
                            return Ok(result);
                        }
                        Ok(None) => {
                            debug!(target: "cover-art", provider = provider.as_str(), "no cover returned from provider");
                        }
                        Err(error) => {
                            warn!(target: "cover-art", provider = provider.as_str(), error = %error, "provider failed");
                            provider_errors.push(ProviderError {
                                provider: CoverArtProvider::CoverArtArchive,
                                message: error.to_string(),
                            });
                        }
                    }
                }
            }
        }

        if provider_attempts > 0 && provider_errors.len() == provider_attempts {
            return Err(CoverArtFallbackError::ProvidersFailed(provider_errors));
        }

        Err(CoverArtFallbackError::NoArtworkFound)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderError {
    pub provider: CoverArtProvider,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum CoverArtFallbackError {
    #[error("All cover-art providers failed")]
    ProvidersFailed(Vec<ProviderError>),
    #[error("No artwork found from configured providers")]
    NoArtworkFound,
    #[error("Rate limiter closed")]
    RateLimiterClosed,
}

#[derive(Debug)]
struct CoverArtArchiveClient {
    client: Client,
    base_url: String,
}

impl CoverArtArchiveClient {
    fn new(base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url
                .unwrap_or_else(|| "https://coverartarchive.org".to_string())
                .trim_end_matches('/')
                .to_string(),
        }
    }

    async fn fetch_album_cover(
        &self,
        release_group_mbid: &str,
    ) -> Result<Option<String>, CoverArtArchiveError> {
        let url = format!("{}/release-group/{}", self.base_url, release_group_mbid);
        debug!(target: "cover-art", url = %url, "fetching cover art from Cover Art Archive");

        let response = self.client.get(url).send().await?;
        let status = response.status();
        let body = response.text().await?;
        let value = parse_cover_art_archive_body(status, &body)?;

        let payload: CoverArtArchiveResponse = serde_json::from_value(value)?;

        let image = payload
            .images
            .iter()
            .find(|image| image.front)
            .and_then(|image| {
                image
                    .thumbnails
                    .large
                    .clone()
                    .or_else(|| image.thumbnails.small.clone())
                    .or_else(|| Some(image.image.clone()))
            })
            .or_else(|| {
                payload
                    .images
                    .first()
                    .map(|image| image.image.clone())
            });

        Ok(image)
    }
}

#[derive(Debug, Error)]
enum CoverArtArchiveError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HTTP status {status}: {body}")]
    HttpStatus { status: StatusCode, body: String },
    #[error("Cover Art Archive API error: {message}")]
    Api { message: String },
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct CoverArtArchiveResponse {
    #[serde(default)]
    images: Vec<CoverArtImage>,
}

#[derive(Debug, Deserialize)]
struct CoverArtImage {
    image: String,
    #[serde(default)]
    front: bool,
    #[serde(default)]
    thumbnails: CoverArtThumbnails,
}

#[derive(Debug, Deserialize, Default)]
struct CoverArtThumbnails {
    #[serde(rename = "250", default)]
    small: Option<String>,
    #[serde(rename = "500", default)]
    large: Option<String>,
}

fn parse_cover_art_archive_body(
    status: StatusCode,
    response_body: &str,
) -> Result<Value, CoverArtArchiveError> {
    if !status.is_success() {
        return Err(CoverArtArchiveError::HttpStatus {
            status,
            body: response_body.to_string(),
        });
    }

    let value: Value = serde_json::from_str(response_body)?;

    if let Some(message) = value
        .get("error")
        .and_then(|error| error.as_str())
        .or_else(|| value.get("message").and_then(|message| message.as_str()))
    {
        return Err(CoverArtArchiveError::Api {
            message: message.to_string(),
        });
    }

    Ok(value)
}
