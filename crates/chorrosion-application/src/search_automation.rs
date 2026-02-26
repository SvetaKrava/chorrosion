use crate::indexers::{IndexerClient, IndexerError, IndexerSearchQuery, IndexerSearchResult};
use crate::release_parsing::{
    deduplicate_releases, filter_releases, parse_release_title, rank_releases, ParsedReleaseTitle,
    ReleaseFilterOptions,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualSearchRequest {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedRelease {
    pub parsed: ParsedReleaseTitle,
    pub search_result: IndexerSearchResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumSearchTarget {
    pub artist: String,
    pub album: String,
    pub already_owned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomaticSearchDecision {
    pub target: AlbumSearchTarget,
    pub best_release: Option<RankedRelease>,
}

pub async fn manual_search<I: IndexerClient>(
    indexer: &I,
    request: &ManualSearchRequest,
    options: &ReleaseFilterOptions,
) -> Result<Vec<RankedRelease>, IndexerError> {
    let query = build_manual_query(request)?;
    let raw_results = indexer
        .search(&IndexerSearchQuery {
            query,
            category: Some("music".to_string()),
            limit: Some(100),
            offset: Some(0),
        })
        .await?;

    Ok(rank_results(raw_results, options))
}

pub async fn automatic_search_missing_albums<I: IndexerClient>(
    indexer: &I,
    targets: &[AlbumSearchTarget],
    options: &ReleaseFilterOptions,
) -> Result<Vec<AutomaticSearchDecision>, IndexerError> {
    let missing_targets = detect_missing_albums(targets);

    let mut decisions = Vec::with_capacity(missing_targets.len());
    for target in missing_targets {
        let raw_results = indexer
            .search(&IndexerSearchQuery {
                query: format!("{} {}", target.artist, target.album),
                category: Some("music".to_string()),
                limit: Some(100),
                offset: Some(0),
            })
            .await?;

        let ranked = rank_results(raw_results, options);
        let best_release = ranked.into_iter().next();

        decisions.push(AutomaticSearchDecision {
            target,
            best_release,
        });
    }

    Ok(decisions)
}

pub fn detect_missing_albums(targets: &[AlbumSearchTarget]) -> Vec<AlbumSearchTarget> {
    targets
        .iter()
        .filter(|target| !target.already_owned)
        .cloned()
        .collect()
}

fn build_manual_query(request: &ManualSearchRequest) -> Result<String, IndexerError> {
    if let Some(query) = request.query.as_deref() {
        let query = query.trim();
        if !query.is_empty() {
            return Ok(query.to_string());
        }
    }

    let mut parts = Vec::new();
    if let Some(artist) = request.artist.as_deref() {
        let artist = artist.trim();
        if !artist.is_empty() {
            parts.push(artist.to_string());
        }
    }
    if let Some(album) = request.album.as_deref() {
        let album = album.trim();
        if !album.is_empty() {
            parts.push(album.to_string());
        }
    }

    if parts.is_empty() {
        return Err(IndexerError::Request(
            "manual search requires either query or artist/album".to_string(),
        ));
    }

    Ok(parts.join(" "))
}

fn rank_results(
    raw_results: Vec<IndexerSearchResult>,
    options: &ReleaseFilterOptions,
) -> Vec<RankedRelease> {
    let parsed_titles: Vec<ParsedReleaseTitle> = raw_results
        .iter()
        .map(|result| parse_release_title(&result.title))
        .collect();

    let filtered = filter_releases(&parsed_titles, options);
    let deduped = deduplicate_releases(&filtered);
    let ranked = rank_releases(deduped, options);

    let mut ranked_releases = Vec::new();
    for parsed in ranked {
        if let Some(search_result) = raw_results
            .iter()
            .find(|result| result.title == parsed.original_title)
            .cloned()
        {
            ranked_releases.push(RankedRelease {
                parsed,
                search_result,
            });
        }
    }

    ranked_releases
}

#[cfg(test)]
mod tests {
    use super::{
        automatic_search_missing_albums, detect_missing_albums, manual_search, AlbumSearchTarget,
        ManualSearchRequest,
    };
    use crate::indexers::{
        IndexerCapabilities, IndexerClient, IndexerConfig, IndexerError, IndexerProtocol,
        IndexerRssItem, IndexerSearchQuery, IndexerSearchResult, IndexerTestResult,
    };
    use crate::release_parsing::{AudioQuality, ReleaseFilterOptions};
    use async_trait::async_trait;

    #[derive(Clone)]
    struct FakeIndexer {
        config: IndexerConfig,
    }

    impl FakeIndexer {
        fn new() -> Self {
            Self {
                config: IndexerConfig {
                    name: "fake".to_string(),
                    base_url: "https://example.invalid".to_string(),
                    protocol: IndexerProtocol::Custom,
                    api_key: None,
                    enabled: true,
                },
            }
        }
    }

    #[async_trait]
    impl IndexerClient for FakeIndexer {
        fn config(&self) -> &IndexerConfig {
            &self.config
        }

        async fn detect_capabilities(&self) -> Result<IndexerCapabilities, IndexerError> {
            Ok(IndexerCapabilities {
                supports_search: true,
                supports_rss: true,
                supports_capabilities_detection: true,
                supports_categories: true,
                supported_categories: vec!["music".to_string()],
            })
        }

        async fn search(
            &self,
            query: &IndexerSearchQuery,
        ) -> Result<Vec<IndexerSearchResult>, IndexerError> {
            if query.query.to_lowercase().contains("daft punk") {
                return Ok(vec![
                    IndexerSearchResult {
                        title: "Daft Punk - Discovery [FLAC]-A".to_string(),
                        guid: Some("1".to_string()),
                        download_url: Some("magnet:?xt=1".to_string()),
                        published_at: None,
                        size_bytes: None,
                        seeders: Some(10),
                        leechers: Some(1),
                    },
                    IndexerSearchResult {
                        title: "Daft Punk - Discovery 320kbps MP3-B".to_string(),
                        guid: Some("2".to_string()),
                        download_url: Some("magnet:?xt=2".to_string()),
                        published_at: None,
                        size_bytes: None,
                        seeders: Some(8),
                        leechers: Some(2),
                    },
                ]);
            }

            if query.query.to_lowercase().contains("radiohead") {
                return Ok(vec![IndexerSearchResult {
                    title: "Radiohead - OK Computer 320kbps MP3-RLS".to_string(),
                    guid: Some("3".to_string()),
                    download_url: Some("magnet:?xt=3".to_string()),
                    published_at: None,
                    size_bytes: None,
                    seeders: Some(4),
                    leechers: Some(1),
                }]);
            }

            Ok(Vec::new())
        }

        async fn fetch_rss_feed(&self) -> Result<Vec<IndexerRssItem>, IndexerError> {
            Ok(Vec::new())
        }

        async fn test_connection(&self) -> Result<IndexerTestResult, IndexerError> {
            Ok(IndexerTestResult {
                success: true,
                message: "ok".to_string(),
                capabilities: None,
            })
        }
    }

    #[test]
    fn detects_missing_targets_only() {
        let targets = vec![
            AlbumSearchTarget {
                artist: "Daft Punk".to_string(),
                album: "Discovery".to_string(),
                already_owned: false,
            },
            AlbumSearchTarget {
                artist: "Radiohead".to_string(),
                album: "OK Computer".to_string(),
                already_owned: true,
            },
        ];

        let missing = detect_missing_albums(&targets);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].artist, "Daft Punk");
    }

    #[tokio::test]
    async fn manual_search_ranks_lossless_above_lossy() {
        let indexer = FakeIndexer::new();
        let request = ManualSearchRequest {
            artist: Some("Daft Punk".to_string()),
            album: Some("Discovery".to_string()),
            query: None,
        };

        let results = manual_search(&indexer, &request, &ReleaseFilterOptions::default())
            .await
            .expect("manual search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].parsed.quality, AudioQuality::Flac);
    }

    #[tokio::test]
    async fn automatic_search_selects_best_release_for_missing_album() {
        let indexer = FakeIndexer::new();
        let targets = vec![AlbumSearchTarget {
            artist: "Radiohead".to_string(),
            album: "OK Computer".to_string(),
            already_owned: false,
        }];

        let decisions = automatic_search_missing_albums(
            &indexer,
            &targets,
            &ReleaseFilterOptions {
                preferred_qualities: vec![AudioQuality::Mp3],
                min_bitrate_kbps: Some(256),
                preferred_release_groups: vec![],
            },
        )
        .await
        .expect("automatic search should succeed");

        assert_eq!(decisions.len(), 1);
        assert!(decisions[0].best_release.is_some());
        assert_eq!(
            decisions[0]
                .best_release
                .as_ref()
                .and_then(|r| r.parsed.album.as_deref()),
            Some("OK Computer")
        );
    }
}
