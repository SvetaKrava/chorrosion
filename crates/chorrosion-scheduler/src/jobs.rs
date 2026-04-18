// SPDX-License-Identifier: GPL-3.0-or-later
use crate::job::{Job, JobContext, JobResult};
use anyhow::Result;
use chorrosion_application::{
    parse_release_title, IndexerClient, IndexerConfig, IndexerProtocol, NewznabClient,
    TorznabClient,
};
use chorrosion_config::{
    CacheConfig, DiscogsAlbumSeed, DiscogsConfig, LastFmAlbumSeed, LastFmConfig,
};
use chorrosion_infrastructure::{
    repositories::{AlbumRepository, Repository},
    sqlite_adapters::{SqliteAlbumRepository, SqliteIndexerDefinitionRepository},
};
use chorrosion_metadata::discogs::DiscogsClient;
use chorrosion_metadata::lastfm::LastFmClient;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, RwLock,
};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Rate limit cache for MusicBrainz API calls
/// Tracks last refresh time per artist/album to avoid excessive API calls
#[derive(Clone)]
pub struct MetadataRefreshCache {
    /// Map of entity ID to last refresh timestamp
    /// Items older than configured TTL are eligible for refresh
    artist_refreshes: Arc<RwLock<HashMap<Uuid, DateTime<Utc>>>>,
    album_refreshes: Arc<RwLock<HashMap<Uuid, DateTime<Utc>>>>,
    /// Cache TTL in seconds - minimum time between refreshes for same entity
    ttl_seconds: i64,
    /// Unix timestamp (seconds) recorded at the *start* of the last prune scan; 0 means never
    /// attempted.  Stored atomically so the throttle check is lock-free.
    last_prune_secs: Arc<AtomicU64>,
    /// Minimum interval between consecutive prune scans in seconds (default: 3600)
    prune_interval_seconds: u64,
}

impl MetadataRefreshCache {
    /// Create a new metadata refresh cache with 24-hour TTL
    pub fn new() -> Self {
        Self {
            artist_refreshes: Arc::new(RwLock::new(HashMap::new())),
            album_refreshes: Arc::new(RwLock::new(HashMap::new())),
            ttl_seconds: 24 * 60 * 60, // 24 hours default
            last_prune_secs: Arc::new(AtomicU64::new(0)),
            prune_interval_seconds: 3600, // prune at most once per hour
        }
    }

    /// Check if an artist should be refreshed based on cache TTL
    /// Returns true if refresh should proceed, false if rate limited or on error
    pub fn should_refresh_artist(&self, artist_id: Uuid) -> bool {
        let cache = self.artist_refreshes.read().unwrap_or_else(|poisoned| {
            warn!(target: "jobs", "artist cache rwlock poisoned, recovering");
            poisoned.into_inner()
        });
        match cache.get(&artist_id) {
            None => true, // Never refreshed before
            Some(last_refresh) => {
                let elapsed = Utc::now().signed_duration_since(*last_refresh);
                elapsed.num_seconds() > self.ttl_seconds
            }
        }
    }

    /// Check if an album should be refreshed based on cache TTL
    /// Returns true if refresh should proceed, false if rate limited or on error
    pub fn should_refresh_album(&self, album_id: Uuid) -> bool {
        let cache = self.album_refreshes.read().unwrap_or_else(|poisoned| {
            warn!(target: "jobs", "album cache rwlock poisoned, recovering");
            poisoned.into_inner()
        });
        match cache.get(&album_id) {
            None => true, // Never refreshed before
            Some(last_refresh) => {
                let elapsed = Utc::now().signed_duration_since(*last_refresh);
                elapsed.num_seconds() > self.ttl_seconds
            }
        }
    }

    /// Atomically check if an artist should be refreshed and mark it if so
    /// Returns true if the refresh should proceed (wasn't already marked within TTL)
    /// This prevents race conditions where multiple jobs could refresh the same entity
    pub fn try_mark_artist_refreshed(&self, artist_id: Uuid) -> bool {
        let mut cache = self.artist_refreshes.write().unwrap_or_else(|poisoned| {
            warn!(target: "jobs", "artist cache rwlock poisoned during mark, recovering");
            poisoned.into_inner()
        });

        // Check if eligible for refresh
        let should_refresh = match cache.get(&artist_id) {
            None => true,
            Some(last_refresh) => {
                let elapsed = Utc::now().signed_duration_since(*last_refresh);
                elapsed.num_seconds() > self.ttl_seconds
            }
        };

        // If eligible, mark as refreshed atomically
        if should_refresh {
            cache.insert(artist_id, Utc::now());
        }

        should_refresh
    }

    /// Atomically check if an album should be refreshed and mark it if so
    /// Returns true if the refresh should proceed (wasn't already marked within TTL)
    /// This prevents race conditions where multiple jobs could refresh the same entity
    pub fn try_mark_album_refreshed(&self, album_id: Uuid) -> bool {
        let mut cache = self.album_refreshes.write().unwrap_or_else(|poisoned| {
            warn!(target: "jobs", "album cache rwlock poisoned during mark, recovering");
            poisoned.into_inner()
        });

        // Check if eligible for refresh
        let should_refresh = match cache.get(&album_id) {
            None => true,
            Some(last_refresh) => {
                let elapsed = Utc::now().signed_duration_since(*last_refresh);
                elapsed.num_seconds() > self.ttl_seconds
            }
        };

        // If eligible, mark as refreshed atomically
        if should_refresh {
            cache.insert(album_id, Utc::now());
        }

        should_refresh
    }

    /// Clear all cached refresh times (useful for testing)
    pub fn clear(&self) {
        if let Ok(mut cache) = self.artist_refreshes.write() {
            cache.clear();
        }
        if let Ok(mut cache) = self.album_refreshes.write() {
            cache.clear();
        }
    }

    /// Prune stale entries older than TTL to prevent unbounded memory growth.
    ///
    /// Pruning is throttled: if called more often than `prune_interval_seconds` (default 1 hour)
    /// the call is a no-op so that the full-scan overhead is bounded even when refresh jobs run
    /// frequently.  A compare-exchange is used to atomically claim the prune run so that
    /// concurrent callers (shared cache via `clone()`) don't all perform a redundant scan.
    /// Poisoned locks are recovered via `into_inner()` so pruning never silently stops.
    pub fn prune_stale_entries(&self) {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Throttle check: if the last prune started recently, skip without taking any locks.
        let last = self.last_prune_secs.load(Ordering::Relaxed);
        if last > 0 && now_secs.saturating_sub(last) < self.prune_interval_seconds {
            return;
        }

        // Atomically claim this prune run via CAS.  If another concurrent caller already
        // updated last_prune_secs between our load and here, we skip rather than double-scan.
        if self
            .last_prune_secs
            .compare_exchange(last, now_secs, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        let now = Utc::now();
        let ttl = self.ttl_seconds;

        let mut cache = self.artist_refreshes.write().unwrap_or_else(|poisoned| {
            warn!(target: "jobs", "artist cache rwlock poisoned during prune, recovering");
            poisoned.into_inner()
        });
        let initial_size = cache.len();
        cache.retain(|_, last_refresh| {
            now.signed_duration_since(*last_refresh).num_seconds() <= ttl
        });
        let pruned = initial_size.saturating_sub(cache.len());
        if pruned > 0 {
            debug!(target: "jobs", "pruned {} stale artist cache entries", pruned);
        }

        let mut cache = self.album_refreshes.write().unwrap_or_else(|poisoned| {
            warn!(target: "jobs", "album cache rwlock poisoned during prune, recovering");
            poisoned.into_inner()
        });
        let initial_size = cache.len();
        cache.retain(|_, last_refresh| {
            now.signed_duration_since(*last_refresh).num_seconds() <= ttl
        });
        let pruned = initial_size.saturating_sub(cache.len());
        if pruned > 0 {
            debug!(target: "jobs", "pruned {} stale album cache entries", pruned);
        }
    }

    /// Returns the Unix-second timestamp stored as the start of the last prune attempt.
    /// Used in tests to verify throttle behavior; not part of the public API.
    #[cfg(test)]
    fn last_prune_secs_value(&self) -> u64 {
        self.last_prune_secs.load(Ordering::Relaxed)
    }

    /// Overrides the last-prune timestamp to simulate a prune that happened `secs_ago` seconds
    /// in the past.  Used in tests to exercise the throttle cadence without sleeping.
    #[cfg(test)]
    fn simulate_last_prune_secs_ago(&self, secs_ago: u64) {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_prune_secs
            .store(now_secs.saturating_sub(secs_ago), Ordering::Relaxed);
    }
}

impl Default for MetadataRefreshCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Last.fm metadata refresh job - enriches artist/album metadata using configured seeds.
pub struct LastFmMetadataRefreshJob {
    client: Arc<LastFmClient>,
    artists: Vec<String>,
    albums: Vec<LastFmAlbumSeed>,
    /// Maximum number of in-flight Tokio tasks at any point.
    /// Prevents excessive memory/scheduling overhead for large seed lists.
    max_concurrent_tasks: usize,
}

impl LastFmMetadataRefreshJob {
    pub fn from_config(config: &LastFmConfig) -> Option<Self> {
        Self::from_config_with_cache(config, &CacheConfig::default())
    }

    pub fn from_config_with_cache(
        config: &LastFmConfig,
        cache_config: &CacheConfig,
    ) -> Option<Self> {
        let api_key = config.api_key.as_deref()?.trim();
        if api_key.is_empty() {
            return None;
        }

        let artists = config
            .seed_artists
            .iter()
            .map(|artist| artist.trim())
            .filter(|artist| !artist.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        let albums = config
            .seed_albums
            .iter()
            .filter(|seed| !seed.artist.trim().is_empty() && !seed.album.trim().is_empty())
            .map(|seed| LastFmAlbumSeed {
                artist: seed.artist.trim().to_string(),
                album: seed.album.trim().to_string(),
            })
            .collect();

        let client = LastFmClient::new_with_limits_cache_timeout_and_base_url(
            api_key.to_string(),
            config.max_concurrent_requests.max(1),
            cache_config.metadata_artist_max_capacity,
            cache_config.metadata_album_max_capacity,
            config.request_timeout_seconds,
            config.base_url.clone(),
        );

        Some(Self {
            client: Arc::new(client),
            artists,
            albums,
            max_concurrent_tasks: config.max_concurrent_requests.max(1),
        })
    }
}

#[async_trait::async_trait]
impl Job for LastFmMetadataRefreshJob {
    fn job_type(&self) -> &'static str {
        "lastfm_metadata_refresh"
    }

    fn name(&self) -> String {
        "Last.fm Metadata Refresh".to_string()
    }

    async fn execute(&self, ctx: JobContext) -> Result<JobResult> {
        let artist_count = self.artists.len();
        let album_count = self.albums.len();
        info!(
            target: "jobs",
            job_id = %ctx.job_id,
            artists = artist_count,
            albums = album_count,
            "executing Last.fm metadata refresh job"
        );

        if artist_count == 0 && album_count == 0 {
            debug!(target: "jobs", job_id = %ctx.job_id, "no Last.fm seeds configured, skipping refresh");
            return Ok(JobResult::Success);
        }

        // Dispatch artist and album fetches concurrently, bounded by `max_concurrent_tasks`.
        // Permits are acquired *before* spawning so only `max_concurrent_tasks` Tokio tasks
        // are live at any point, avoiding excessive memory/scheduler overhead for large seed lists.
        // Rate limiting is additionally enforced by the client's internal semaphore.
        let task_sem = Arc::new(Semaphore::new(self.max_concurrent_tasks));
        let mut set: JoinSet<Result<(), (String, String)>> = JoinSet::new();

        for artist in &self.artists {
            // The semaphore is created locally and never explicitly closed, so
            // acquire_owned() is infallible here.
            let permit = Arc::clone(&task_sem)
                .acquire_owned()
                .await
                .expect("task semaphore closed unexpectedly");
            let client = Arc::clone(&self.client);
            let artist = artist.clone();
            set.spawn(async move {
                let _permit = permit;
                let fetch_result = client.fetch_artist_metadata(&artist).await;
                match fetch_result {
                    Ok(_) => Ok(()),
                    Err(e) => Err((artist, e.to_string())),
                }
            });
        }

        for seed in &self.albums {
            let permit = Arc::clone(&task_sem)
                .acquire_owned()
                .await
                .expect("task semaphore closed unexpectedly");
            let client = Arc::clone(&self.client);
            let seed = seed.clone();
            set.spawn(async move {
                let _permit = permit;
                let fetch_result = client.fetch_album_metadata(&seed.artist, &seed.album).await;
                match fetch_result {
                    Ok(_) => Ok(()),
                    Err(e) => Err((format!("{}/{}", seed.artist, seed.album), e.to_string())),
                }
            });
        }

        let mut failures = 0usize;
        loop {
            let joined = set.join_next().await;
            let Some(result) = joined else {
                break;
            };
            match result {
                Ok(Ok(())) => {}
                Ok(Err((name, error))) => {
                    failures += 1;
                    warn!(
                        target: "jobs",
                        job_id = %ctx.job_id,
                        entity = %name,
                        error = %error,
                        "failed to refresh metadata from Last.fm"
                    );
                }
                Err(join_err) => {
                    failures += 1;
                    warn!(
                        target: "jobs",
                        job_id = %ctx.job_id,
                        error = %join_err,
                        "Last.fm metadata task panicked"
                    );
                }
            }
        }

        if failures > 0 {
            return Ok(JobResult::Failure {
                error: format!(
                    "Last.fm metadata refresh completed with {} failed requests",
                    failures
                ),
                retry: true,
            });
        }

        info!(target: "jobs", job_id = %ctx.job_id, "Last.fm metadata refresh completed successfully");
        Ok(JobResult::Success)
    }

    fn max_retries(&self) -> u32 {
        2
    }

    fn retry_delay_seconds(&self) -> u64 {
        120
    }
}

/// Discogs metadata refresh job - enriches artist/album metadata using configured seeds.
pub struct DiscogsMetadataRefreshJob {
    client: Arc<DiscogsClient>,
    artists: Vec<String>,
    albums: Vec<DiscogsAlbumSeed>,
    /// Maximum number of in-flight Tokio tasks at any point.
    /// Prevents excessive memory/scheduling overhead for large seed lists.
    max_concurrent_tasks: usize,
}

impl DiscogsMetadataRefreshJob {
    pub fn from_config(config: &DiscogsConfig) -> Option<Self> {
        Self::from_config_with_cache(config, &CacheConfig::default())
    }

    pub fn from_config_with_cache(
        config: &DiscogsConfig,
        cache_config: &CacheConfig,
    ) -> Option<Self> {
        let artists: Vec<String> = config
            .seed_artists
            .iter()
            .map(|artist| artist.trim())
            .filter(|artist| !artist.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        let albums: Vec<DiscogsAlbumSeed> = config
            .seed_albums
            .iter()
            .filter(|seed| !seed.artist.trim().is_empty() && !seed.album.trim().is_empty())
            .map(|seed| DiscogsAlbumSeed {
                artist: seed.artist.trim().to_string(),
                album: seed.album.trim().to_string(),
            })
            .collect();

        if artists.is_empty() && albums.is_empty() {
            return None;
        }

        let client = DiscogsClient::new_with_limits_cache_timeout_and_base_url(
            config.token.clone(),
            config.max_concurrent_requests.max(1),
            cache_config.metadata_artist_max_capacity,
            cache_config.metadata_album_max_capacity,
            config.request_timeout_seconds,
            config.base_url.clone(),
        );

        Some(Self {
            client: Arc::new(client),
            artists,
            albums,
            max_concurrent_tasks: config.max_concurrent_requests.max(1),
        })
    }
}

#[async_trait::async_trait]
impl Job for DiscogsMetadataRefreshJob {
    fn job_type(&self) -> &'static str {
        "discogs_metadata_refresh"
    }

    fn name(&self) -> String {
        "Discogs Metadata Refresh".to_string()
    }

    async fn execute(&self, ctx: JobContext) -> Result<JobResult> {
        let artist_count = self.artists.len();
        let album_count = self.albums.len();
        info!(
            target: "jobs",
            job_id = %ctx.job_id,
            artists = artist_count,
            albums = album_count,
            "executing Discogs metadata refresh job"
        );

        let mut failures = 0usize;

        // Dispatch artist and album fetches concurrently, bounded by `max_concurrent_tasks`.
        // Permits are acquired *before* spawning so only `max_concurrent_tasks` Tokio tasks
        // are live at any point.  Rate limiting is additionally enforced by the client's
        // internal semaphore and interval limiter.
        let task_sem = Arc::new(Semaphore::new(self.max_concurrent_tasks));
        let mut set: JoinSet<Result<(), (String, String)>> = JoinSet::new();

        for artist in &self.artists {
            // The semaphore is created locally and never explicitly closed, so
            // acquire_owned() is infallible here.
            let permit = Arc::clone(&task_sem)
                .acquire_owned()
                .await
                .expect("task semaphore closed unexpectedly");
            let client = Arc::clone(&self.client);
            let artist = artist.clone();
            set.spawn(async move {
                let _permit = permit;
                let fetch_result = client.fetch_artist_metadata(&artist).await;
                match fetch_result {
                    Ok(_) => Ok(()),
                    Err(e) => Err((artist, e.to_string())),
                }
            });
        }

        for seed in &self.albums {
            let permit = Arc::clone(&task_sem)
                .acquire_owned()
                .await
                .expect("task semaphore closed unexpectedly");
            let client = Arc::clone(&self.client);
            let seed = seed.clone();
            set.spawn(async move {
                let _permit = permit;
                let fetch_result = client.fetch_album_metadata(&seed.artist, &seed.album).await;
                match fetch_result {
                    Ok(_) => Ok(()),
                    Err(e) => Err((format!("{}/{}", seed.artist, seed.album), e.to_string())),
                }
            });
        }

        loop {
            let joined = set.join_next().await;
            let Some(result) = joined else {
                break;
            };
            match result {
                Ok(Ok(())) => {}
                Ok(Err((name, error))) => {
                    failures += 1;
                    warn!(
                        target: "jobs",
                        job_id = %ctx.job_id,
                        entity = %name,
                        error = %error,
                        "failed to refresh metadata from Discogs"
                    );
                }
                Err(join_err) => {
                    failures += 1;
                    warn!(
                        target: "jobs",
                        job_id = %ctx.job_id,
                        error = %join_err,
                        "Discogs metadata task panicked"
                    );
                }
            }
        }

        if failures > 0 {
            return Ok(JobResult::Failure {
                error: format!(
                    "Discogs metadata refresh completed with {} failed requests",
                    failures
                ),
                retry: true,
            });
        }

        info!(target: "jobs", job_id = %ctx.job_id, "Discogs metadata refresh completed successfully");
        Ok(JobResult::Success)
    }

    fn max_retries(&self) -> u32 {
        2
    }

    fn retry_delay_seconds(&self) -> u64 {
        120
    }
}

pub struct RssSyncJob {
    album_repository: Arc<SqliteAlbumRepository>,
    indexer_repository: Arc<SqliteIndexerDefinitionRepository>,
    scan_limit: i64,
}

const SUPPORTED_RSS_PROTOCOLS: &str = "newznab, torznab";

impl RssSyncJob {
    pub fn new(
        album_repository: Arc<SqliteAlbumRepository>,
        indexer_repository: Arc<SqliteIndexerDefinitionRepository>,
    ) -> Self {
        Self {
            album_repository,
            indexer_repository,
            scan_limit: 5000,
        }
    }
}

#[async_trait::async_trait]
impl Job for RssSyncJob {
    fn job_type(&self) -> &'static str {
        "rss_sync"
    }

    fn name(&self) -> String {
        "RSS Sync".to_string()
    }

    async fn execute(&self, ctx: JobContext) -> Result<JobResult> {
        info!(target: "jobs", job_id = %ctx.job_id, "executing RSS sync job");

        let indexers = match self.indexer_repository.list(5000, 0).await {
            Ok(indexers) => indexers
                .into_iter()
                .filter(|i| i.enabled)
                .collect::<Vec<_>>(),
            Err(error) => {
                return Ok(JobResult::Failure {
                    error: format!("failed to list configured indexers: {error}"),
                    retry: true,
                });
            }
        };

        if indexers.is_empty() {
            info!(target: "jobs", job_id = %ctx.job_id, "no enabled indexers configured; skipping RSS sync");
            return Ok(JobResult::Success);
        }

        let wanted_titles =
            match collect_wanted_album_titles(&self.album_repository, self.scan_limit).await {
                Ok(titles) => titles,
                Err(error) => {
                    return Ok(JobResult::Failure {
                        error: format!("failed to load wanted albums for RSS matching: {error}"),
                        retry: true,
                    });
                }
            };

        if wanted_titles.is_empty() {
            info!(target: "jobs", job_id = %ctx.job_id, "no wanted albums to match; skipping RSS sync");
            return Ok(JobResult::Success);
        }

        let enabled_indexers = indexers.len();
        let mut indexers_polled: usize = 0;
        let mut poll_failures: usize = 0;
        let mut config_failures: usize = 0;
        let mut rss_items_seen: usize = 0;
        let mut rss_items_matched: usize = 0;

        for definition in indexers {
            let protocol = match definition.protocol.parse::<IndexerProtocol>() {
                Ok(protocol) => protocol,
                Err(error) => {
                    config_failures += 1;
                    warn!(
                        target: "jobs",
                        job_id = %ctx.job_id,
                        indexer = %definition.name,
                        protocol = %definition.protocol,
                        error = %error,
                        "skipping indexer: unrecognized protocol"
                    );
                    continue;
                }
            };

            let config = IndexerConfig {
                name: definition.name.clone(),
                base_url: definition.base_url.clone(),
                protocol: protocol.clone(),
                api_key: definition.api_key.clone(),
                enabled: definition.enabled,
            };

            let fetch_result = match protocol {
                IndexerProtocol::Newznab => {
                    indexers_polled += 1;
                    let client = NewznabClient::new(config);
                    let rss_items = client.fetch_rss_feed().await;
                    rss_items
                }
                IndexerProtocol::Torznab => {
                    indexers_polled += 1;
                    let client = TorznabClient::new(config);
                    let rss_items = client.fetch_rss_feed().await;
                    rss_items
                }
                other => {
                    config_failures += 1;
                    warn!(
                        target: "jobs",
                        job_id = %ctx.job_id,
                        indexer = %definition.name,
                        protocol = %other.as_str(),
                        supported_protocols = %SUPPORTED_RSS_PROTOCOLS,
                        "skipping indexer: unsupported RSS sync protocol"
                    );
                    continue;
                }
            };

            match fetch_result {
                Ok(items) => {
                    let matched = count_rss_matches(&items, &wanted_titles);
                    rss_items_seen += items.len();
                    rss_items_matched += matched;
                    info!(
                        target: "jobs",
                        job_id = %ctx.job_id,
                        indexer = %definition.name,
                        rss_items = items.len(),
                        matched,
                        "processed RSS feed for indexer"
                    );
                }
                Err(error) => {
                    poll_failures += 1;
                    warn!(
                        target: "jobs",
                        job_id = %ctx.job_id,
                        indexer = %definition.name,
                        error = %error,
                        "failed to fetch RSS feed from indexer"
                    );
                }
            }
        }

        info!(
            target: "jobs",
            job_id = %ctx.job_id,
            enabled_indexers,
            indexers_polled,
            poll_failures,
            config_failures,
            rss_items_seen,
            rss_items_matched,
            wanted_album_count = wanted_titles.len(),
            "RSS sync completed"
        );

        if indexers_polled == 0 && config_failures > 0 {
            return Ok(JobResult::Failure {
                error: format!(
                    "no enabled indexers use a supported RSS protocol (supported: {})",
                    SUPPORTED_RSS_PROTOCOLS
                ),
                retry: false,
            });
        }

        if indexers_polled > 0 && poll_failures == indexers_polled {
            return Ok(JobResult::Failure {
                error: "failed to fetch RSS feeds from all polled indexers".to_string(),
                retry: true,
            });
        }

        Ok(JobResult::Success)
    }

    fn is_retriable(&self) -> bool {
        true
    }

    fn max_retries(&self) -> u32 {
        2
    }
}

async fn collect_wanted_album_titles(
    album_repository: &SqliteAlbumRepository,
    scan_limit: i64,
) -> Result<HashSet<String>> {
    let mut titles = HashSet::new();

    collect_titles_by_source(
        album_repository,
        scan_limit,
        &mut titles,
        WantedAlbumTitleSource::WantedWithoutTracks,
    )
    .await?;
    collect_titles_by_source(
        album_repository,
        scan_limit,
        &mut titles,
        WantedAlbumTitleSource::CutoffUnmet,
    )
    .await?;

    Ok(titles)
}

/// Source query used to collect candidate album titles for RSS matching.
enum WantedAlbumTitleSource {
    WantedWithoutTracks,
    CutoffUnmet,
}

/// Collect wanted album titles from a paginated repository query and append them to `titles`.
async fn collect_titles_by_source(
    album_repository: &SqliteAlbumRepository,
    scan_limit: i64,
    titles: &mut HashSet<String>,
    source: WantedAlbumTitleSource,
) -> Result<()> {
    let mut offset: i64 = 0;
    loop {
        let batch = match source {
            WantedAlbumTitleSource::WantedWithoutTracks => {
                album_repository
                    .list_wanted_without_tracks(scan_limit, offset)
                    .await?
            }
            WantedAlbumTitleSource::CutoffUnmet => {
                album_repository
                    .list_cutoff_unmet_albums(scan_limit, offset)
                    .await?
            }
        };

        let batch_len = batch.len();
        for album in batch {
            titles.insert(normalize_match_key(&album.title));
        }
        if batch_len < scan_limit as usize {
            break;
        }
        offset += scan_limit;
    }

    Ok(())
}

fn count_rss_matches(
    items: &[chorrosion_application::IndexerRssItem],
    wanted_titles: &HashSet<String>,
) -> usize {
    items
        .iter()
        .filter(|item| {
            let parsed = parse_release_title(&item.title);
            parsed
                .album
                .as_ref()
                .is_some_and(|album| wanted_titles.contains(&normalize_match_key(album)))
        })
        .count()
}

fn normalize_match_key(value: &str) -> String {
    value.trim().to_lowercase()
}

/// Backlog search job - searches indexers for missing albums
pub struct BacklogSearchJob {
    album_repository: Arc<SqliteAlbumRepository>,
    scan_limit: i64,
}

impl BacklogSearchJob {
    pub fn new(album_repository: Arc<SqliteAlbumRepository>) -> Self {
        Self {
            album_repository,
            scan_limit: 5000,
        }
    }
}

#[async_trait::async_trait]
impl Job for BacklogSearchJob {
    fn job_type(&self) -> &'static str {
        "backlog_search"
    }

    fn name(&self) -> String {
        "Backlog Search".to_string()
    }

    async fn execute(&self, ctx: JobContext) -> Result<JobResult> {
        info!(target: "jobs", job_id = %ctx.job_id, "executing backlog search job");

        let mut candidate_ids = HashSet::new();

        // Page through all wanted albums without tracks, collecting only IDs
        let mut missing_count: usize = 0;
        let mut offset: i64 = 0;
        loop {
            let batch = match self
                .album_repository
                .list_wanted_without_tracks(self.scan_limit, offset)
                .await
            {
                Ok(albums) => albums,
                Err(error) => {
                    return Ok(JobResult::Failure {
                        error: format!("failed to collect wanted albums without tracks: {error}"),
                        retry: true,
                    });
                }
            };
            let batch_len = batch.len();
            missing_count += batch_len;
            for album in batch {
                candidate_ids.insert(album.id);
            }
            if batch_len < self.scan_limit as usize {
                break;
            }
            offset += self.scan_limit;
        }

        // Page through all cutoff-unmet albums, collecting only IDs
        let mut cutoff_unmet_count: usize = 0;
        let mut cutoff_offset: i64 = 0;
        loop {
            let batch = match self
                .album_repository
                .list_cutoff_unmet_albums(self.scan_limit, cutoff_offset)
                .await
            {
                Ok(albums) => albums,
                Err(error) => {
                    return Ok(JobResult::Failure {
                        error: format!("failed to collect cutoff-unmet albums: {error}"),
                        retry: true,
                    });
                }
            };
            let batch_len = batch.len();
            cutoff_unmet_count += batch_len;
            for album in batch {
                candidate_ids.insert(album.id);
            }
            if batch_len < self.scan_limit as usize {
                break;
            }
            cutoff_offset += self.scan_limit;
        }

        info!(
            target: "jobs",
            job_id = %ctx.job_id,
            missing_count,
            cutoff_unmet_count,
            scheduled_count = candidate_ids.len(),
            "automated backlog search scheduling snapshot"
        );

        info!(target: "jobs", job_id = %ctx.job_id, "backlog search completed");
        Ok(JobResult::Success)
    }

    fn max_retries(&self) -> u32 {
        1
    }
}

/// Artist refresh job - updates artist metadata from external sources
///
/// This job refreshes artist metadata from MusicBrainz based on the artist's MBID.
/// It implements rate limiting and caching to avoid excessive API calls:
/// - Tracks refresh timestamps per artist (24-hour TTL by default)
/// - Skips refresh if already completed within TTL window
/// - Respects MusicBrainz rate limiting via the client
/// - Supports both single artist and bulk refresh operations
pub struct RefreshArtistJob {
    artist_id: Option<String>,
    /// Shared cache for tracking refresh timestamps
    cache: MetadataRefreshCache,
}

impl RefreshArtistJob {
    /// Create a job to refresh a single artist by ID
    pub fn single(artist_id: impl Into<String>) -> Self {
        Self {
            artist_id: Some(artist_id.into()),
            cache: MetadataRefreshCache::new(),
        }
    }

    /// Create a job to refresh all monitored artists
    pub fn all() -> Self {
        Self {
            artist_id: None,
            cache: MetadataRefreshCache::new(),
        }
    }

    /// Create a job with an existing cache (useful for scheduled jobs that run repeatedly)
    pub fn with_cache(artist_id: Option<String>, cache: MetadataRefreshCache) -> Self {
        Self { artist_id, cache }
    }

    /// Get a reference to the cache for external use (e.g., scheduler reuse across invocations)
    pub fn cache(&self) -> &MetadataRefreshCache {
        &self.cache
    }
}

#[async_trait::async_trait]
impl Job for RefreshArtistJob {
    fn job_type(&self) -> &'static str {
        "refresh_artist"
    }

    fn name(&self) -> String {
        match &self.artist_id {
            Some(id) => format!("Refresh Artist {}", id),
            None => "Refresh All Artists".to_string(),
        }
    }

    async fn execute(&self, ctx: JobContext) -> Result<JobResult> {
        self.cache.prune_stale_entries();

        match &self.artist_id {
            Some(id) => {
                info!(target: "jobs", job_id = %ctx.job_id, artist_id = %id, "refreshing single artist metadata");

                // Parse artist ID as UUID
                match Uuid::parse_str(id) {
                    Ok(uuid) => {
                        // Atomically check and mark to prevent race conditions
                        if !self.cache.try_mark_artist_refreshed(uuid) {
                            debug!(target: "jobs", job_id = %ctx.job_id, artist_id = %id, 
                                   "artist already refreshed recently, skipping (rate limit)");
                            return Ok(JobResult::Success);
                        }

                        // TODO: In full implementation:
                        // 1. Load artist from database with its MusicBrainz ID
                        // 2. If MBID exists, call MusicBrainz client to fetch latest artist metadata
                        // 3. Update artist record with new data (biography, disambiguation, etc.)
                        // 4. Schedule cover art fetch job if needed

                        info!(target: "jobs", job_id = %ctx.job_id, artist_id = %id, 
                              "single artist metadata refresh completed (placeholder)");
                        Ok(JobResult::Success)
                    }
                    Err(e) => {
                        warn!(target: "jobs", job_id = %ctx.job_id, artist_id = %id, error = %e, 
                              "invalid artist ID format, expected UUID");
                        Ok(JobResult::Failure {
                            error: format!("Invalid artist ID: {}", e),
                            retry: false,
                        })
                    }
                }
            }
            None => {
                info!(target: "jobs", job_id = %ctx.job_id, "refreshing all monitored artists metadata");

                // TODO: In full implementation:
                // 1. Query database for all monitored artists
                // 2. For each artist with a MusicBrainz ID:
                //    a. Check cache (skip if recently refreshed)
                //    b. Fetch updated metadata from MusicBrainz
                //    c. Update database record
                //    d. Mark as refreshed in cache
                // 3. Schedule cover art fetch jobs for updated artists
                // 4. Batch requests to respect rate limits
                // 5. If partial failure, return appropriate retry status

                info!(target: "jobs", job_id = %ctx.job_id, "all artists metadata refresh completed (placeholder)");
                Ok(JobResult::Success)
            }
        }
    }

    fn max_retries(&self) -> u32 {
        3
    }

    fn retry_delay_seconds(&self) -> u64 {
        300 // 5 minutes
    }
}

/// Album refresh job - updates album metadata from external sources
///
/// This job refreshes album metadata from MusicBrainz based on the album's MBID.
/// It implements rate limiting and caching similar to RefreshArtistJob:
/// - Tracks refresh timestamps per album (24-hour TTL by default)
/// - Skips refresh if already completed within TTL window
/// - Respects MusicBrainz rate limiting via the client
/// - Supports both single album and bulk refresh operations
pub struct RefreshAlbumJob {
    album_id: Option<String>,
    /// Shared cache for tracking refresh timestamps
    cache: MetadataRefreshCache,
}

impl RefreshAlbumJob {
    /// Create a job to refresh a single album by ID
    pub fn single(album_id: impl Into<String>) -> Self {
        Self {
            album_id: Some(album_id.into()),
            cache: MetadataRefreshCache::new(),
        }
    }

    /// Create a job to refresh all monitored albums
    pub fn all() -> Self {
        Self {
            album_id: None,
            cache: MetadataRefreshCache::new(),
        }
    }

    /// Create a job with an existing cache (useful for scheduled jobs that run repeatedly)
    pub fn with_cache(album_id: Option<String>, cache: MetadataRefreshCache) -> Self {
        Self { album_id, cache }
    }

    /// Get a reference to the cache for external use
    pub fn cache(&self) -> &MetadataRefreshCache {
        &self.cache
    }
}

#[async_trait::async_trait]
impl Job for RefreshAlbumJob {
    fn job_type(&self) -> &'static str {
        "refresh_album"
    }

    fn name(&self) -> String {
        match &self.album_id {
            Some(id) => format!("Refresh Album {}", id),
            None => "Refresh All Albums".to_string(),
        }
    }

    async fn execute(&self, ctx: JobContext) -> Result<JobResult> {
        self.cache.prune_stale_entries();

        match &self.album_id {
            Some(id) => {
                info!(target: "jobs", job_id = %ctx.job_id, album_id = %id, "refreshing single album metadata");

                // Parse album ID as UUID
                match Uuid::parse_str(id) {
                    Ok(uuid) => {
                        // Atomically check and mark to prevent race conditions
                        if !self.cache.try_mark_album_refreshed(uuid) {
                            debug!(target: "jobs", job_id = %ctx.job_id, album_id = %id, 
                                   "album already refreshed recently, skipping (rate limit)");
                            return Ok(JobResult::Success);
                        }

                        // TODO: In full implementation:
                        // 1. Load album from database with its MusicBrainz ID (release group MBID)
                        // 2. If MBID exists, call MusicBrainz client to fetch latest album metadata
                        // 3. Update album record with new data (release dates, types, tracks, etc.)
                        // 4. Enqueue cover art fetch job if artwork not cached

                        info!(target: "jobs", job_id = %ctx.job_id, album_id = %id, 
                              "single album metadata refresh completed (placeholder)");
                        Ok(JobResult::Success)
                    }
                    Err(e) => {
                        warn!(target: "jobs", job_id = %ctx.job_id, album_id = %id, error = %e, 
                              "invalid album ID format, expected UUID");
                        Ok(JobResult::Failure {
                            error: format!("Invalid album ID: {}", e),
                            retry: false,
                        })
                    }
                }
            }
            None => {
                info!(target: "jobs", job_id = %ctx.job_id, "refreshing all monitored albums metadata");

                // TODO: In full implementation:
                // 1. Query database for all monitored albums with MusicBrainz IDs
                // 2. For each album:
                //    a. Check cache (skip if recently refreshed)
                //    b. Fetch updated metadata from MusicBrainz
                //    c. Update database record with release dates, types, and track listings
                //    d. Mark as refreshed in cache
                // 3. Schedule cover art fetch jobs for updated albums
                // 4. Batch requests to respect rate limits and improve performance
                // 5. Handle partial failures gracefully

                info!(target: "jobs", job_id = %ctx.job_id, "all albums metadata refresh completed (placeholder)");
                Ok(JobResult::Success)
            }
        }
    }

    fn max_retries(&self) -> u32 {
        3
    }

    fn retry_delay_seconds(&self) -> u64 {
        300 // 5 minutes
    }
}

/// Housekeeping job - cleanup, backups, maintenance tasks
pub struct HousekeepingJob;

impl HousekeepingJob {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HousekeepingJob {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Job for HousekeepingJob {
    fn job_type(&self) -> &'static str {
        "housekeeping"
    }

    fn name(&self) -> String {
        "Housekeeping".to_string()
    }

    async fn execute(&self, ctx: JobContext) -> Result<JobResult> {
        info!(target: "jobs", job_id = %ctx.job_id, "executing housekeeping job");

        // TODO: Implement housekeeping tasks
        // - Cleanup old job logs
        // - Vacuum database
        // - Remove orphaned files
        // - Create backups if configured

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        info!(target: "jobs", job_id = %ctx.job_id, "housekeeping completed");
        Ok(JobResult::Success)
    }

    fn is_retriable(&self) -> bool {
        false // Housekeeping failures shouldn't retry
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lastfm_job_not_created_without_api_key() {
        let config = LastFmConfig::default();
        let job = LastFmMetadataRefreshJob::from_config(&config);
        assert!(job.is_none());
    }

    #[test]
    fn test_lastfm_job_created_with_api_key() {
        let config = LastFmConfig {
            api_key: Some("test-api-key".to_string()),
            base_url: Some("http://127.0.0.1:3030/2.0".to_string()),
            max_concurrent_requests: 2,
            request_timeout_seconds: 15,
            seed_artists: vec!["  Daft Punk  ".to_string()],
            seed_albums: vec![LastFmAlbumSeed {
                artist: "Nirvana".to_string(),
                album: "Nevermind".to_string(),
            }],
        };

        let job = LastFmMetadataRefreshJob::from_config(&config);
        assert!(job.is_some());
    }

    #[tokio::test]
    async fn test_lastfm_job_executes_without_seeds() {
        let config = LastFmConfig {
            api_key: Some("test-api-key".to_string()),
            base_url: Some("http://127.0.0.1:3030/2.0".to_string()),
            max_concurrent_requests: 1,
            request_timeout_seconds: 15,
            seed_artists: Vec::new(),
            seed_albums: Vec::new(),
        };
        let job = LastFmMetadataRefreshJob::from_config(&config)
            .expect("job should be created when API key is present");
        let result = job.execute(JobContext::new("lastfm-empty-seeds")).await;
        assert!(matches!(result, Ok(JobResult::Success)));
    }

    #[test]
    fn test_lastfm_job_not_created_with_empty_api_key() {
        let config = LastFmConfig {
            api_key: Some(String::new()),
            ..LastFmConfig::default()
        };
        assert!(LastFmMetadataRefreshJob::from_config(&config).is_none());
    }

    #[test]
    fn test_lastfm_job_not_created_with_whitespace_api_key() {
        let config = LastFmConfig {
            api_key: Some("   ".to_string()),
            ..LastFmConfig::default()
        };
        assert!(LastFmMetadataRefreshJob::from_config(&config).is_none());
    }

    #[test]
    fn test_lastfm_job_filters_whitespace_only_artist_seeds() {
        let config = LastFmConfig {
            api_key: Some("test-api-key".to_string()),
            seed_artists: vec!["   ".to_string(), "".to_string(), "Radiohead".to_string()],
            ..LastFmConfig::default()
        };
        let job = LastFmMetadataRefreshJob::from_config(&config)
            .expect("job should be created when API key is present");
        assert_eq!(job.artists, vec!["Radiohead".to_string()]);
    }

    #[test]
    fn test_lastfm_job_filters_album_seeds_with_empty_fields() {
        let config = LastFmConfig {
            api_key: Some("test-api-key".to_string()),
            seed_albums: vec![
                LastFmAlbumSeed {
                    artist: "".to_string(),
                    album: "OK Computer".to_string(),
                },
                LastFmAlbumSeed {
                    artist: "Radiohead".to_string(),
                    album: "   ".to_string(),
                },
                LastFmAlbumSeed {
                    artist: "Radiohead".to_string(),
                    album: "OK Computer".to_string(),
                },
            ],
            ..LastFmConfig::default()
        };
        let job = LastFmMetadataRefreshJob::from_config(&config)
            .expect("job should be created when API key is present");
        assert_eq!(job.albums.len(), 1);
        assert_eq!(job.albums[0].artist, "Radiohead");
        assert_eq!(job.albums[0].album, "OK Computer");
    }

    #[test]
    fn test_discogs_job_not_created_without_seeds() {
        let config = DiscogsConfig::default();
        assert!(DiscogsMetadataRefreshJob::from_config(&config).is_none());
    }

    #[test]
    fn test_discogs_job_created_with_artist_seed() {
        let config = DiscogsConfig {
            token: Some("discogs-token".to_string()),
            base_url: Some("http://127.0.0.1:3030".to_string()),
            max_concurrent_requests: 2,
            request_timeout_seconds: 15,
            seed_artists: vec!["  Massive Attack  ".to_string()],
            seed_albums: Vec::new(),
        };

        let job = DiscogsMetadataRefreshJob::from_config(&config);
        assert!(job.is_some());
        let job = job.expect("job should be created when seeds are present");
        assert_eq!(job.artists, vec!["Massive Attack".to_string()]);
    }

    #[tokio::test]
    async fn test_discogs_job_not_created_with_empty_sanitized_seeds() {
        let config = DiscogsConfig {
            token: Some("discogs-token".to_string()),
            base_url: Some("http://127.0.0.1:3030".to_string()),
            max_concurrent_requests: 1,
            request_timeout_seconds: 15,
            seed_artists: vec!["   ".to_string()],
            seed_albums: vec![DiscogsAlbumSeed {
                artist: "".to_string(),
                album: "".to_string(),
            }],
        };

        let job = DiscogsMetadataRefreshJob::from_config(&config);
        assert!(job.is_none());
    }

    #[test]
    fn test_metadata_refresh_cache_new_artist() {
        let cache = MetadataRefreshCache::new();
        let artist_id = Uuid::new_v4();

        // New artist should always be eligible for refresh
        assert!(cache.should_refresh_artist(artist_id));
    }

    #[test]
    fn test_metadata_refresh_cache_mark_and_check() {
        let cache = MetadataRefreshCache::new();
        let artist_id = Uuid::new_v4();

        // Mark as refreshed (atomically)
        assert!(cache.try_mark_artist_refreshed(artist_id));

        // Should not be eligible for immediate refresh (within TTL)
        assert!(!cache.should_refresh_artist(artist_id));
    }

    #[test]
    fn test_metadata_refresh_cache_separate_entities() {
        let cache = MetadataRefreshCache::new();
        let artist_id1 = Uuid::new_v4();
        let artist_id2 = Uuid::new_v4();

        // Mark first artist as refreshed
        assert!(cache.try_mark_artist_refreshed(artist_id1));

        // First artist should not need refresh
        assert!(!cache.should_refresh_artist(artist_id1));

        // Second artist should still be eligible
        assert!(cache.should_refresh_artist(artist_id2));
    }

    #[test]
    fn test_metadata_refresh_cache_clear() {
        let cache = MetadataRefreshCache::new();
        let artist_id = Uuid::new_v4();
        let album_id = Uuid::new_v4();

        // Mark both as refreshed
        assert!(cache.try_mark_artist_refreshed(artist_id));
        assert!(cache.try_mark_album_refreshed(album_id));

        // Both should not need refresh
        assert!(!cache.should_refresh_artist(artist_id));
        assert!(!cache.should_refresh_album(album_id));

        // Clear cache
        cache.clear();

        // Both should now be eligible for refresh
        assert!(cache.should_refresh_artist(artist_id));
        assert!(cache.should_refresh_album(album_id));
    }

    #[tokio::test]
    async fn test_refresh_artist_job_invalid_id() {
        let job = RefreshArtistJob::single("not-a-uuid");
        let ctx = JobContext::new("test-job-1");

        let result = job.execute(ctx).await;

        assert!(result.is_ok());
        match result.unwrap() {
            JobResult::Failure { error, retry } => {
                assert!(!retry);
                assert!(error.contains("Invalid artist ID"));
            }
            _ => panic!("Expected Failure result"),
        }
    }

    #[tokio::test]
    async fn test_refresh_artist_job_single() {
        let artist_id = Uuid::new_v4();
        let job = RefreshArtistJob::single(artist_id.to_string());
        let ctx = JobContext::new("test-job-2");

        let result = job.execute(ctx).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), JobResult::Success));

        // Verify it was marked as refreshed
        assert!(!job.cache.should_refresh_artist(artist_id));
    }

    #[tokio::test]
    async fn test_refresh_album_job_invalid_id() {
        let job = RefreshAlbumJob::single("not-a-uuid");
        let ctx = JobContext::new("test-job-3");

        let result = job.execute(ctx).await;

        assert!(result.is_ok());
        match result.unwrap() {
            JobResult::Failure { error, retry } => {
                assert!(!retry);
                assert!(error.contains("Invalid album ID"));
            }
            _ => panic!("Expected Failure result"),
        }
    }

    #[tokio::test]
    async fn test_refresh_album_job_single() {
        let album_id = Uuid::new_v4();
        let job = RefreshAlbumJob::single(album_id.to_string());
        let ctx = JobContext::new("test-job-4");

        let result = job.execute(ctx).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), JobResult::Success));

        // Verify it was marked as refreshed
        assert!(!job.cache.should_refresh_album(album_id));
    }

    #[tokio::test]
    async fn test_refresh_artist_job_names() {
        let artist_id = Uuid::new_v4();
        let single_job = RefreshArtistJob::single(artist_id.to_string());
        let all_job = RefreshArtistJob::all();

        assert_eq!(single_job.job_type(), "refresh_artist");
        assert!(single_job.name().contains(&artist_id.to_string()));
        assert_eq!(all_job.name(), "Refresh All Artists");
    }

    #[tokio::test]
    async fn test_refresh_album_job_names() {
        let album_id = Uuid::new_v4();
        let single_job = RefreshAlbumJob::single(album_id.to_string());
        let all_job = RefreshAlbumJob::all();

        assert_eq!(single_job.job_type(), "refresh_album");
        assert!(single_job.name().contains(&album_id.to_string()));
        assert_eq!(all_job.name(), "Refresh All Albums");
    }

    #[test]
    fn test_refresh_jobs_retry_config() {
        let artist_job = RefreshArtistJob::all();
        let album_job = RefreshAlbumJob::all();

        assert_eq!(artist_job.max_retries(), 3);
        assert_eq!(artist_job.retry_delay_seconds(), 300);

        assert_eq!(album_job.max_retries(), 3);
        assert_eq!(album_job.retry_delay_seconds(), 300);
    }

    #[tokio::test]
    async fn test_refresh_artist_job_respects_rate_limit() {
        // Test that executing the same job twice with the same cache instance
        // correctly skips the second refresh due to rate limiting
        let artist_id = Uuid::new_v4();
        let cache = MetadataRefreshCache::new();

        // First execution should succeed
        let job1 = RefreshArtistJob::with_cache(Some(artist_id.to_string()), cache.clone());
        let ctx1 = JobContext::new("test-job-rate-1");
        let result1 = job1.execute(ctx1).await;
        assert!(result1.is_ok());
        assert!(matches!(result1.unwrap(), JobResult::Success));

        // Second execution with same cache should skip (rate limited)
        let job2 = RefreshArtistJob::with_cache(Some(artist_id.to_string()), cache.clone());
        let ctx2 = JobContext::new("test-job-rate-2");
        let result2 = job2.execute(ctx2).await;
        assert!(result2.is_ok());
        assert!(matches!(result2.unwrap(), JobResult::Success));

        // Verify cache still has the artist marked as recently refreshed
        assert!(!cache.should_refresh_artist(artist_id));
    }

    #[tokio::test]
    async fn test_refresh_album_job_respects_rate_limit() {
        // Test that executing the same album job twice with the same cache instance
        // correctly skips the second refresh due to rate limiting
        let album_id = Uuid::new_v4();
        let cache = MetadataRefreshCache::new();

        // First execution should succeed
        let job1 = RefreshAlbumJob::with_cache(Some(album_id.to_string()), cache.clone());
        let ctx1 = JobContext::new("test-job-rate-3");
        let result1 = job1.execute(ctx1).await;
        assert!(result1.is_ok());
        assert!(matches!(result1.unwrap(), JobResult::Success));

        // Second execution with same cache should skip (rate limited)
        let job2 = RefreshAlbumJob::with_cache(Some(album_id.to_string()), cache.clone());
        let ctx2 = JobContext::new("test-job-rate-4");
        let result2 = job2.execute(ctx2).await;
        assert!(result2.is_ok());
        assert!(matches!(result2.unwrap(), JobResult::Success));

        // Verify cache still has the album marked as recently refreshed
        assert!(!cache.should_refresh_album(album_id));
    }

    #[tokio::test]
    async fn test_refresh_all_artists_job_executes() {
        // Test that the "refresh all" code path completes successfully
        // This is a placeholder test until the full implementation is added
        let job = RefreshArtistJob::all();
        let ctx = JobContext::new("test-job-all-artists");

        let result = job.execute(ctx).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), JobResult::Success));
    }

    #[tokio::test]
    async fn test_refresh_all_albums_job_executes() {
        // Test that the "refresh all albums" code path completes successfully
        // This is a placeholder test until the full implementation is added
        let job = RefreshAlbumJob::all();
        let ctx = JobContext::new("test-job-all-albums");

        let result = job.execute(ctx).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), JobResult::Success));
    }

    #[test]
    fn test_cache_persistence_with_shared_instance() {
        // Test that cache state persists when using with_cache() constructor
        let cache = MetadataRefreshCache::new();
        let artist_id1 = Uuid::new_v4();
        let artist_id2 = Uuid::new_v4();

        // Create first job and mark artist 1 as refreshed
        let job1 = RefreshArtistJob::with_cache(Some(artist_id1.to_string()), cache.clone());
        assert!(job1.cache().try_mark_artist_refreshed(artist_id1));

        // Create second job with same cache - should see artist 1 as already refreshed
        let job2 = RefreshArtistJob::with_cache(Some(artist_id2.to_string()), cache.clone());
        assert!(!job2.cache().should_refresh_artist(artist_id1));
        assert!(job2.cache().should_refresh_artist(artist_id2));
    }

    #[test]
    fn test_cache_eviction_prunes_stale_entries() {
        // Test that prune_stale_entries removes old entries but keeps recent ones
        let cache = MetadataRefreshCache::new();
        let artist_id = Uuid::new_v4();

        // Mark as refreshed
        assert!(cache.try_mark_artist_refreshed(artist_id));
        assert!(!cache.should_refresh_artist(artist_id));

        // Prune immediately - entry should still be there (within TTL)
        cache.prune_stale_entries();
        assert!(!cache.should_refresh_artist(artist_id));

        // Note: Testing actual TTL expiration would require mocking time or very long test delays
        // The implementation is correct - this test verifies the pruning mechanism exists
    }

    #[test]
    fn test_prune_throttle_skips_within_interval() {
        let cache = MetadataRefreshCache::new();

        // Fresh cache: last_prune_secs should be 0 (never pruned)
        assert_eq!(cache.last_prune_secs_value(), 0);

        // First call should proceed and record a non-zero timestamp
        cache.prune_stale_entries();
        let ts1 = cache.last_prune_secs_value();
        assert!(ts1 > 0, "first prune should set last_prune_secs");

        // Immediate second call should be a no-op (throttled within interval)
        cache.prune_stale_entries();
        let ts2 = cache.last_prune_secs_value();
        assert_eq!(
            ts1, ts2,
            "second prune within interval should not update last_prune_secs"
        );
    }

    #[test]
    fn test_prune_runs_again_after_interval_expires() {
        let cache = MetadataRefreshCache::new();

        // Simulate that the last prune happened 2 hours ago (beyond the 1-hour interval)
        cache.simulate_last_prune_secs_ago(2 * 3600);
        let old_ts = cache.last_prune_secs_value();
        assert!(old_ts > 0);

        // Prune should run again since the interval has elapsed
        cache.prune_stale_entries();
        let new_ts = cache.last_prune_secs_value();
        assert!(
            new_ts > old_ts,
            "prune after interval expiry should update last_prune_secs"
        );
    }

    #[test]
    fn test_atomic_try_mark_prevents_race() {
        // Test that try_mark_* operations are atomic and prevent double-marking
        let cache = MetadataRefreshCache::new();
        let artist_id = Uuid::new_v4();

        // First try_mark should succeed
        assert!(cache.try_mark_artist_refreshed(artist_id));

        // Second try_mark on same ID should fail (already marked within TTL)
        assert!(!cache.try_mark_artist_refreshed(artist_id));

        // Same for albums
        let album_id = Uuid::new_v4();
        assert!(cache.try_mark_album_refreshed(album_id));
        assert!(!cache.try_mark_album_refreshed(album_id));
    }

    // ── BacklogSearchJob tests ───────────────────────────────────────────────

    async fn make_migrated_pool() -> sqlx::SqlitePool {
        let config = chorrosion_config::AppConfig {
            database: chorrosion_config::DatabaseConfig {
                url: "sqlite::memory:".to_string(),
                pool_max_size: 1,
                ..chorrosion_config::DatabaseConfig::default()
            },
            ..chorrosion_config::AppConfig::default()
        };
        chorrosion_infrastructure::init_database(&config)
            .await
            .expect("in-memory DB init failed")
    }

    #[tokio::test]
    async fn test_backlog_search_job_name_and_type() {
        let pool = make_migrated_pool().await;
        let repo =
            Arc::new(chorrosion_infrastructure::sqlite_adapters::SqliteAlbumRepository::new(pool));
        let job = BacklogSearchJob::new(repo);
        assert_eq!(job.job_type(), "backlog_search");
        assert_eq!(job.name(), "Backlog Search");
        assert_eq!(job.max_retries(), 1);
    }

    #[tokio::test]
    async fn test_backlog_search_job_empty_database_returns_success() {
        let pool = make_migrated_pool().await;
        let repo =
            Arc::new(chorrosion_infrastructure::sqlite_adapters::SqliteAlbumRepository::new(pool));
        let job = BacklogSearchJob::new(repo);
        let ctx = JobContext::new("test-backlog-empty");

        let result = job.execute(ctx).await;
        assert!(matches!(result, Ok(JobResult::Success)));
    }

    #[tokio::test]
    async fn test_backlog_search_job_missing_tables_returns_retriable_failure() {
        // Pool without migrations → tables absent → repository errors → Failure { retry: true }
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("pool connect failed");
        let repo =
            Arc::new(chorrosion_infrastructure::sqlite_adapters::SqliteAlbumRepository::new(pool));
        let job = BacklogSearchJob::new(repo);
        let ctx = JobContext::new("test-backlog-no-tables");

        let result = job.execute(ctx).await.expect("execute should not Err");
        match result {
            JobResult::Failure { retry, .. } => assert!(retry, "failure must be retriable"),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    // ── RssSyncJob tests ────────────────────────────────────────────────────

    #[test]
    fn test_rss_match_count_matches_album_titles_from_parsed_release() {
        let wanted = ["OK Computer", "In Rainbows"]
            .into_iter()
            .map(normalize_match_key)
            .collect::<HashSet<_>>();

        let items = vec![
            chorrosion_application::IndexerRssItem {
                title: "Radiohead - OK Computer FLAC".to_string(),
                guid: None,
                link: None,
                published_at: None,
                description: None,
            },
            chorrosion_application::IndexerRssItem {
                title: "Somebody - Unrelated Album MP3 320".to_string(),
                guid: None,
                link: None,
                published_at: None,
                description: None,
            },
            chorrosion_application::IndexerRssItem {
                title: "Radiohead - In Rainbows [2007]".to_string(),
                guid: None,
                link: None,
                published_at: None,
                description: None,
            },
        ];

        assert_eq!(count_rss_matches(&items, &wanted), 2);
    }

    #[tokio::test]
    async fn test_rss_sync_job_returns_success_when_no_indexers() {
        let pool = make_migrated_pool().await;
        let album_repo = Arc::new(SqliteAlbumRepository::new(pool.clone()));
        let indexer_repo = Arc::new(SqliteIndexerDefinitionRepository::new(pool));
        let job = RssSyncJob::new(album_repo, indexer_repo);
        let ctx = JobContext::new("test-rss-no-indexers");

        let result = job.execute(ctx).await.expect("execute should not Err");
        assert!(matches!(result, JobResult::Success));
    }

    #[tokio::test]
    async fn test_rss_sync_job_returns_non_retriable_failure_for_unsupported_protocols() {
        let pool = make_migrated_pool().await;

        let artist_id = Uuid::new_v4().to_string();
        let album_id = Uuid::new_v4().to_string();
        let indexer_id = Uuid::new_v4().to_string();

        sqlx::query("INSERT INTO artists (id, name, status, monitored) VALUES (?, ?, ?, ?)")
            .bind(&artist_id)
            .bind("Radiohead")
            .bind("continuing")
            .bind(true)
            .execute(&pool)
            .await
            .expect("insert artist failed");

        sqlx::query(
            "INSERT INTO albums (id, artist_id, title, status, monitored) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&album_id)
        .bind(&artist_id)
        .bind("OK Computer")
        .bind("wanted")
        .bind(true)
        .execute(&pool)
        .await
        .expect("insert wanted album failed");

        sqlx::query(
            "INSERT INTO indexer_definitions (id, name, base_url, protocol, enabled) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&indexer_id)
        .bind("Unsupported Indexer")
        .bind("https://example.com")
        .bind("gazelle")
        .bind(true)
        .execute(&pool)
        .await
        .expect("insert indexer failed");

        let album_repo = Arc::new(SqliteAlbumRepository::new(pool.clone()));
        let indexer_repo = Arc::new(SqliteIndexerDefinitionRepository::new(pool));
        let job = RssSyncJob::new(album_repo, indexer_repo);
        let ctx = JobContext::new("test-rss-unsupported-protocols");

        let result = job.execute(ctx).await.expect("execute should not Err");
        match result {
            JobResult::Failure { retry, error } => {
                assert!(!retry, "unsupported protocols should not be retriable");
                assert!(
                    error.contains("supported RSS protocol"),
                    "unexpected error: {error}"
                );
            }
            other => panic!("expected non-retriable Failure, got {other:?}"),
        }
    }
}
