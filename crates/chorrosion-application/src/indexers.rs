// SPDX-License-Identifier: GPL-3.0-or-later
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use quick_xml::de::from_str;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexerProtocol {
    Newznab,
    Torznab,
    Gazelle,
    Custom,
}

impl IndexerProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Newznab => "newznab",
            Self::Torznab => "torznab",
            Self::Gazelle => "gazelle",
            Self::Custom => "custom",
        }
    }
}

impl std::str::FromStr for IndexerProtocol {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "newznab" => Ok(Self::Newznab),
            "torznab" => Ok(Self::Torznab),
            "gazelle" => Ok(Self::Gazelle),
            "custom" => Ok(Self::Custom),
            other => Err(format!("unknown indexer protocol: '{other}'")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexerConfig {
    pub name: String,
    pub base_url: String,
    pub protocol: IndexerProtocol,
    pub api_key: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexerCapabilities {
    pub supports_search: bool,
    pub supports_rss: bool,
    pub supports_capabilities_detection: bool,
    pub supports_categories: bool,
    pub supported_categories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexerTestResult {
    pub success: bool,
    pub message: String,
    pub capabilities: Option<IndexerCapabilities>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexerSearchQuery {
    pub query: String,
    pub category: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexerSearchResult {
    pub title: String,
    pub guid: Option<String>,
    pub download_url: Option<String>,
    pub published_at: Option<String>,
    pub size_bytes: Option<u64>,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexerRssItem {
    pub title: String,
    pub guid: Option<String>,
    pub link: Option<String>,
    pub published_at: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Error)]
pub enum IndexerError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("capability detection failed: {0}")]
    Capabilities(String),
    #[error("rss parse error: {0}")]
    RssParse(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}

#[async_trait]
pub trait IndexerClient: Send + Sync {
    fn config(&self) -> &IndexerConfig;

    async fn detect_capabilities(&self) -> Result<IndexerCapabilities, IndexerError>;

    async fn search(
        &self,
        query: &IndexerSearchQuery,
    ) -> Result<Vec<IndexerSearchResult>, IndexerError>;

    async fn fetch_rss_feed(&self) -> Result<Vec<IndexerRssItem>, IndexerError>;

    async fn test_connection(&self) -> Result<IndexerTestResult, IndexerError>;
}

pub struct NewznabClient {
    config: IndexerConfig,
    client: Client,
}

impl NewznabClient {
    pub fn new(config: IndexerConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

pub struct TorznabClient {
    config: IndexerConfig,
    client: Client,
}

impl TorznabClient {
    pub fn new(config: IndexerConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl IndexerClient for NewznabClient {
    fn config(&self) -> &IndexerConfig {
        &self.config
    }

    async fn detect_capabilities(&self) -> Result<IndexerCapabilities, IndexerError> {
        detect_capabilities(&self.client, &self.config).await
    }

    async fn search(
        &self,
        query: &IndexerSearchQuery,
    ) -> Result<Vec<IndexerSearchResult>, IndexerError> {
        let xml = execute_search(&self.client, &self.config, query).await?;
        parse_search_results(&xml)
    }

    async fn fetch_rss_feed(&self) -> Result<Vec<IndexerRssItem>, IndexerError> {
        let xml = execute_search(
            &self.client,
            &self.config,
            &IndexerSearchQuery {
                query: String::new(),
                category: Some("music".to_string()),
                limit: Some(50),
                offset: None,
            },
        )
        .await?;
        parse_rss_feed(&xml)
    }

    async fn test_connection(&self) -> Result<IndexerTestResult, IndexerError> {
        let capabilities = self.detect_capabilities().await?;
        Ok(IndexerTestResult {
            success: true,
            message: format!("Indexer '{}' connection successful", self.config.name),
            capabilities: Some(capabilities),
        })
    }
}

#[async_trait]
impl IndexerClient for TorznabClient {
    fn config(&self) -> &IndexerConfig {
        &self.config
    }

    async fn detect_capabilities(&self) -> Result<IndexerCapabilities, IndexerError> {
        detect_capabilities(&self.client, &self.config).await
    }

    async fn search(
        &self,
        query: &IndexerSearchQuery,
    ) -> Result<Vec<IndexerSearchResult>, IndexerError> {
        let xml = execute_search(&self.client, &self.config, query).await?;
        parse_search_results(&xml)
    }

    async fn fetch_rss_feed(&self) -> Result<Vec<IndexerRssItem>, IndexerError> {
        let xml = execute_search(
            &self.client,
            &self.config,
            &IndexerSearchQuery {
                query: String::new(),
                category: Some("music".to_string()),
                limit: Some(50),
                offset: None,
            },
        )
        .await?;
        parse_rss_feed(&xml)
    }

    async fn test_connection(&self) -> Result<IndexerTestResult, IndexerError> {
        let capabilities = self.detect_capabilities().await?;
        Ok(IndexerTestResult {
            success: true,
            message: format!("Indexer '{}' connection successful", self.config.name),
            capabilities: Some(capabilities),
        })
    }
}

async fn detect_capabilities(
    client: &Client,
    config: &IndexerConfig,
) -> Result<IndexerCapabilities, IndexerError> {
    let xml = execute_api_request(client, config, "caps", None).await?;
    let supports_search = xml.contains("search") || xml.contains("<searching>");
    let supports_rss = true;
    let supports_capabilities_detection = xml.contains("<caps") || xml.contains("<categories");
    let supports_categories = xml.contains("<category");

    let mut supported_categories = Vec::new();
    if supports_categories {
        for token in ["music", "audio/flac", "audio/mp3"] {
            if xml.to_lowercase().contains(token) {
                supported_categories.push(token.to_string());
            }
        }
    }

    if supported_categories.is_empty() {
        supported_categories = vec!["music".to_string(), "audio/flac".to_string()];
    }

    Ok(IndexerCapabilities {
        supports_search,
        supports_rss,
        supports_capabilities_detection,
        supports_categories,
        supported_categories,
    })
}

async fn execute_search(
    client: &Client,
    config: &IndexerConfig,
    query: &IndexerSearchQuery,
) -> Result<String, IndexerError> {
    let mut params: Vec<(&str, String)> = vec![("t", "search".to_string())];

    if !query.query.trim().is_empty() {
        params.push(("q", query.query.trim().to_string()));
    }

    if let Some(category) = query.category.as_deref() {
        params.push((
            "cat",
            map_category_to_indexer(category, &config.protocol).to_string(),
        ));
    }

    if let Some(limit) = query.limit {
        params.push(("limit", limit.to_string()));
    }

    if let Some(offset) = query.offset {
        params.push(("offset", offset.to_string()));
    }

    execute_api_request(client, config, "search", Some(params)).await
}

fn map_category_to_indexer(category: &str, protocol: &IndexerProtocol) -> &'static str {
    let normalized = category.trim().to_lowercase();
    match (protocol, normalized.as_str()) {
        (IndexerProtocol::Newznab | IndexerProtocol::Torznab, "music") => "3000",
        (IndexerProtocol::Newznab | IndexerProtocol::Torznab, "audio/mp3") => "3010",
        (IndexerProtocol::Newznab | IndexerProtocol::Torznab, "audio/flac") => "3040",
        _ => "3000",
    }
}

async fn execute_api_request(
    client: &Client,
    config: &IndexerConfig,
    request_type: &str,
    extra_params: Option<Vec<(&str, String)>>,
) -> Result<String, IndexerError> {
    let mut url = Url::parse(&config.base_url)
        .map_err(|error| IndexerError::Request(format!("invalid base url: {error}")))?;
    if !url.path().ends_with("/api") {
        url.set_path("/api");
    }

    let mut query_pairs: Vec<(&str, String)> = vec![("t", request_type.to_string())];
    if let Some(api_key) = config.api_key.as_deref() {
        if !api_key.trim().is_empty() {
            query_pairs.push(("apikey", api_key.trim().to_string()));
        }
    }
    if let Some(extra) = extra_params {
        query_pairs.extend(extra);
    }

    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query_pairs {
            pairs.append_pair(key, &value);
        }
    }

    debug!(target: "indexers", url = %url, protocol = %config.protocol.as_str(), "requesting indexer endpoint");

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| IndexerError::Request(error.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| IndexerError::Request(error.to_string()))?;

    if !status.is_success() {
        return Err(IndexerError::Request(format!(
            "status {}: {}",
            status.as_u16(),
            body
        )));
    }

    Ok(body)
}

pub fn parse_search_results(xml: &str) -> Result<Vec<IndexerSearchResult>, IndexerError> {
    let envelope: SearchEnvelope =
        from_str(xml).map_err(|error| IndexerError::RssParse(error.to_string()))?;

    Ok(envelope
        .channel
        .items
        .into_iter()
        .map(|item| {
            let mut seeders = None;
            let mut leechers = None;
            let mut size_bytes = item.enclosure.as_ref().and_then(|e| e.length);
            for attr in &item.attributes {
                match attr.name.as_str() {
                    "seeders" => seeders = attr.value.parse::<u32>().ok(),
                    "peers" | "leechers" => leechers = attr.value.parse::<u32>().ok(),
                    "size" if size_bytes.is_none() => size_bytes = attr.value.parse::<u64>().ok(),
                    _ => {}
                }
            }

            let download_url = item
                .enclosure
                .as_ref()
                .and_then(|e| e.url.clone())
                .or_else(|| item.link.clone());

            IndexerSearchResult {
                title: item.title,
                guid: item.guid,
                download_url,
                published_at: parse_pub_date(item.pub_date),
                size_bytes,
                seeders,
                leechers,
            }
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct SearchEnvelope {
    channel: SearchChannel,
}

#[derive(Debug, Deserialize)]
struct SearchChannel {
    #[serde(rename = "item", default)]
    items: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    title: String,
    guid: Option<String>,
    link: Option<String>,
    #[serde(rename = "pubDate")]
    pub_date: Option<String>,
    enclosure: Option<SearchEnclosure>,
    #[serde(rename = "torznab:attr", alias = "attr", default)]
    attributes: Vec<TorznabAttribute>,
}

#[derive(Debug, Deserialize)]
struct SearchEnclosure {
    #[serde(rename = "@url")]
    url: Option<String>,
    #[serde(rename = "@length")]
    length: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TorznabAttribute {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@value")]
    value: String,
}

pub fn parse_rss_feed(xml: &str) -> Result<Vec<IndexerRssItem>, IndexerError> {
    let envelope: RssEnvelope =
        from_str(xml).map_err(|error| IndexerError::RssParse(error.to_string()))?;

    Ok(envelope
        .channel
        .items
        .into_iter()
        .map(|item| IndexerRssItem {
            title: item.title,
            guid: item.guid,
            link: item.link,
            published_at: parse_pub_date(item.pub_date),
            description: item.description,
        })
        .collect())
}

fn parse_pub_date(value: Option<String>) -> Option<String> {
    let date = value?;

    if let Ok(parsed) = DateTime::parse_from_rfc2822(&date) {
        return Some(parsed.with_timezone(&Utc).to_rfc3339());
    }

    if let Ok(parsed) = DateTime::parse_from_rfc3339(&date) {
        return Some(parsed.with_timezone(&Utc).to_rfc3339());
    }

    Some(date)
}

#[derive(Debug, Deserialize)]
struct RssEnvelope {
    channel: RssChannel,
}

#[derive(Debug, Deserialize)]
struct RssChannel {
    #[serde(rename = "item", default)]
    items: Vec<RssRawItem>,
}

#[derive(Debug, Deserialize)]
struct RssRawItem {
    title: String,
    guid: Option<String>,
    link: Option<String>,
    #[serde(rename = "pubDate")]
    pub_date: Option<String>,
    description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        parse_rss_feed, parse_search_results, IndexerClient, IndexerConfig, IndexerProtocol,
        IndexerSearchQuery, NewznabClient, TorznabClient,
    };
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parses_rss_items() {
        let xml = r#"
            <rss>
                <channel>
                    <item>
                        <title>Artist - Album FLAC</title>
                        <guid>abc-123</guid>
                        <link>https://example.org/download/abc</link>
                        <pubDate>Wed, 25 Feb 2026 10:00:00 +0000</pubDate>
                        <description>Lossless release</description>
                    </item>
                </channel>
            </rss>
        "#;

        let items = parse_rss_feed(xml).expect("rss should parse");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Artist - Album FLAC");
        assert_eq!(items[0].guid.as_deref(), Some("abc-123"));
        assert_eq!(
            items[0].link.as_deref(),
            Some("https://example.org/download/abc")
        );
        assert_eq!(
            items[0].published_at.as_deref(),
            Some("2026-02-25T10:00:00+00:00")
        );
    }

    #[test]
    fn errors_on_invalid_rss() {
        let result = parse_rss_feed("<rss><broken></rss>");
        assert!(result.is_err());
    }

    #[test]
    fn parses_search_results_with_torznab_attributes() {
        let xml = r#"
            <rss>
              <channel>
                <item>
                  <title>Artist - Album [FLAC]</title>
                  <guid>guid-1</guid>
                  <link>https://indexer.example/download/1</link>
                  <pubDate>Wed, 25 Feb 2026 10:00:00 +0000</pubDate>
                  <enclosure url="magnet:?xt=urn:btih:123" length="123456789" type="application/x-bittorrent" />
                  <torznab:attr name="seeders" value="42" />
                  <torznab:attr name="leechers" value="7" />
                </item>
              </channel>
            </rss>
        "#;

        let results = parse_search_results(xml).expect("search results should parse");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Artist - Album [FLAC]");
        assert_eq!(results[0].guid.as_deref(), Some("guid-1"));
        assert_eq!(
            results[0].download_url.as_deref(),
            Some("magnet:?xt=urn:btih:123")
        );
        assert_eq!(results[0].size_bytes, Some(123_456_789));
        assert_eq!(results[0].seeders, Some(42));
        assert_eq!(results[0].leechers, Some(7));
    }

    #[tokio::test]
    async fn newznab_search_uses_music_category_mapping() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api"))
            .and(query_param("t", "search"))
            .and(query_param("q", "nirvana"))
            .and(query_param("cat", "3000"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"<rss><channel><item><title>Nevermind FLAC</title><guid>n-1</guid><link>https://example.com/nzb</link></item></channel></rss>"#,
            ))
            .mount(&server)
            .await;

        let client = NewznabClient::new(IndexerConfig {
            name: "test-newznab".to_string(),
            base_url: server.uri(),
            protocol: IndexerProtocol::Newznab,
            api_key: Some("secret".to_string()),
            enabled: true,
        });

        let results = client
            .search(&IndexerSearchQuery {
                query: "nirvana".to_string(),
                category: Some("music".to_string()),
                limit: None,
                offset: None,
            })
            .await
            .expect("newznab search should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Nevermind FLAC");
    }

    #[tokio::test]
    async fn torznab_search_prefers_magnet_from_enclosure() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api"))
            .and(query_param("t", "search"))
            .and(query_param("q", "radiohead"))
            .and(query_param("cat", "3040"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"
                <rss>
                  <channel>
                    <item>
                      <title>Kid A FLAC</title>
                      <guid>t-1</guid>
                      <link>https://example.com/torrent/1</link>
                      <enclosure url="magnet:?xt=urn:btih:abcdef" length="54321" type="application/x-bittorrent" />
                      <torznab:attr name="seeders" value="99" />
                    </item>
                  </channel>
                </rss>
                "#,
            ))
            .mount(&server)
            .await;

        let client = TorznabClient::new(IndexerConfig {
            name: "test-torznab".to_string(),
            base_url: server.uri(),
            protocol: IndexerProtocol::Torznab,
            api_key: None,
            enabled: true,
        });

        let results = client
            .search(&IndexerSearchQuery {
                query: "radiohead".to_string(),
                category: Some("audio/flac".to_string()),
                limit: None,
                offset: None,
            })
            .await
            .expect("torznab search should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].download_url.as_deref(),
            Some("magnet:?xt=urn:btih:abcdef")
        );
        assert_eq!(results[0].seeders, Some(99));
    }

    #[tokio::test]
    async fn protocol_client_can_fetch_rss_feed() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api"))
            .and(query_param("t", "search"))
            .and(query_param("cat", "3000"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"
                <rss>
                  <channel>
                    <item>
                      <title>Weekly Release</title>
                      <guid>rss-1</guid>
                      <link>https://example.com/rss/1</link>
                      <pubDate>Wed, 25 Feb 2026 11:00:00 +0000</pubDate>
                    </item>
                  </channel>
                </rss>
                "#,
            ))
            .mount(&server)
            .await;

        let client = NewznabClient::new(IndexerConfig {
            name: "rss-newznab".to_string(),
            base_url: server.uri(),
            protocol: IndexerProtocol::Newznab,
            api_key: None,
            enabled: true,
        });

        let rss_items = client
            .fetch_rss_feed()
            .await
            .expect("rss fetch should succeed");

        assert_eq!(rss_items.len(), 1);
        assert_eq!(rss_items[0].title, "Weekly Release");
    }
}
