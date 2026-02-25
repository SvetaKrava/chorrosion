use async_trait::async_trait;
use chrono::{DateTime, Utc};
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

    pub fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "newznab" => Self::Newznab,
            "torznab" => Self::Torznab,
            "gazelle" => Self::Gazelle,
            _ => Self::Custom,
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
    use super::parse_rss_feed;

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
}
