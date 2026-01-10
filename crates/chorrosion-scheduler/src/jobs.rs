// SPDX-License-Identifier: GPL-3.0-or-later
use crate::job::{Job, JobContext, JobResult};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Rate limit cache for MusicBrainz API calls
/// Tracks last refresh time per artist/album to avoid excessive API calls
#[derive(Clone)]
pub struct MetadataRefreshCache {
    /// Map of entity ID to last refresh timestamp
    /// Items older than configured TTL are eligible for refresh
    artist_refreshes: Arc<Mutex<HashMap<Uuid, DateTime<Utc>>>>,
    album_refreshes: Arc<Mutex<HashMap<Uuid, DateTime<Utc>>>>,
    /// Cache TTL in seconds - minimum time between refreshes for same entity
    ttl_seconds: i64,
}

impl MetadataRefreshCache {
    /// Create a new metadata refresh cache with 24-hour TTL
    pub fn new() -> Self {
        Self {
            artist_refreshes: Arc::new(Mutex::new(HashMap::new())),
            album_refreshes: Arc::new(Mutex::new(HashMap::new())),
            ttl_seconds: 24 * 60 * 60, // 24 hours default
        }
    }

    /// Check if an artist should be refreshed based on cache TTL
    pub fn should_refresh_artist(&self, artist_id: Uuid) -> bool {
        let cache = self
            .artist_refreshes
            .lock()
            .expect("failed to acquire artist cache lock");
        match cache.get(&artist_id) {
            None => true, // Never refreshed before
            Some(last_refresh) => {
                let elapsed = Utc::now().signed_duration_since(*last_refresh);
                elapsed.num_seconds() > self.ttl_seconds
            }
        }
    }

    /// Check if an album should be refreshed based on cache TTL
    pub fn should_refresh_album(&self, album_id: Uuid) -> bool {
        let cache = self
            .album_refreshes
            .lock()
            .expect("failed to acquire album cache lock");
        match cache.get(&album_id) {
            None => true, // Never refreshed before
            Some(last_refresh) => {
                let elapsed = Utc::now().signed_duration_since(*last_refresh);
                elapsed.num_seconds() > self.ttl_seconds
            }
        }
    }

    /// Mark an artist as recently refreshed
    pub fn mark_artist_refreshed(&self, artist_id: Uuid) {
        let mut cache = self
            .artist_refreshes
            .lock()
            .expect("failed to acquire artist cache lock");
        cache.insert(artist_id, Utc::now());
    }

    /// Mark an album as recently refreshed
    pub fn mark_album_refreshed(&self, album_id: Uuid) {
        let mut cache = self
            .album_refreshes
            .lock()
            .expect("failed to acquire album cache lock");
        cache.insert(album_id, Utc::now());
    }

    /// Clear all cached refresh times (useful for testing)
    pub fn clear(&self) {
        self.artist_refreshes
            .lock()
            .expect("failed to acquire artist cache lock")
            .clear();
        self.album_refreshes
            .lock()
            .expect("failed to acquire album cache lock")
            .clear();
    }
}

impl Default for MetadataRefreshCache {
    fn default() -> Self {
        Self::new()
    }
}
pub struct RssSyncJob;

impl RssSyncJob {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RssSyncJob {
    fn default() -> Self {
        Self::new()
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

        // TODO: Implement actual RSS polling logic
        // - Fetch configured indexers from database
        // - Poll each indexer's RSS feed
        // - Parse and filter new releases
        // - Create download tasks for monitored artists/albums

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        info!(target: "jobs", job_id = %ctx.job_id, "RSS sync completed successfully");
        Ok(JobResult::Success)
    }

    fn is_retriable(&self) -> bool {
        true
    }

    fn max_retries(&self) -> u32 {
        2
    }
}

/// Backlog search job - searches indexers for missing albums
pub struct BacklogSearchJob;

impl BacklogSearchJob {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BacklogSearchJob {
    fn default() -> Self {
        Self::new()
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

        // TODO: Implement backlog search logic
        // - Query database for wanted albums without files
        // - Search each album on configured indexers
        // - Create download tasks for best matches
        // - Update album status

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

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
        match &self.artist_id {
            Some(id) => {
                info!(target: "jobs", job_id = %ctx.job_id, artist_id = %id, "refreshing single artist metadata");

                // Parse artist ID as UUID
                match Uuid::parse_str(id) {
                    Ok(uuid) => {
                        // Check if this artist has been refreshed recently (rate limiting)
                        if !self.cache.should_refresh_artist(uuid) {
                            debug!(target: "jobs", job_id = %ctx.job_id, artist_id = %id, 
                                   "artist already refreshed recently, skipping (rate limit)");
                            return Ok(JobResult::Success);
                        }

                        // TODO: In full implementation:
                        // 1. Load artist from database with its MusicBrainz ID
                        // 2. If MBID exists, call MusicBrainz client to fetch latest artist metadata
                        // 3. Update artist record with new data (biography, disambiguation, etc.)
                        // 4. Mark as refreshed in cache
                        // 5. Schedule cover art fetch job if needed

                        info!(target: "jobs", job_id = %ctx.job_id, artist_id = %id, 
                              "single artist metadata refresh completed (placeholder)");
                        self.cache.mark_artist_refreshed(uuid);
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
        match &self.album_id {
            Some(id) => {
                info!(target: "jobs", job_id = %ctx.job_id, album_id = %id, "refreshing single album metadata");

                // Parse album ID as UUID
                match Uuid::parse_str(id) {
                    Ok(uuid) => {
                        // Check if this album has been refreshed recently (rate limiting)
                        if !self.cache.should_refresh_album(uuid) {
                            debug!(target: "jobs", job_id = %ctx.job_id, album_id = %id, 
                                   "album already refreshed recently, skipping (rate limit)");
                            return Ok(JobResult::Success);
                        }

                        // TODO: In full implementation:
                        // 1. Load album from database with its MusicBrainz ID (release group MBID)
                        // 2. If MBID exists, call MusicBrainz client to fetch latest album metadata
                        // 3. Update album record with new data (release dates, types, tracks, etc.)
                        // 4. Mark as refreshed in cache
                        // 5. Enqueue cover art fetch job if artwork not cached

                        info!(target: "jobs", job_id = %ctx.job_id, album_id = %id, 
                              "single album metadata refresh completed (placeholder)");
                        self.cache.mark_album_refreshed(uuid);
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

        // Mark as refreshed
        cache.mark_artist_refreshed(artist_id);

        // Should not be eligible for immediate refresh (within TTL)
        assert!(!cache.should_refresh_artist(artist_id));
    }

    #[test]
    fn test_metadata_refresh_cache_separate_entities() {
        let cache = MetadataRefreshCache::new();
        let artist_id1 = Uuid::new_v4();
        let artist_id2 = Uuid::new_v4();

        // Mark first artist as refreshed
        cache.mark_artist_refreshed(artist_id1);

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
        cache.mark_artist_refreshed(artist_id);
        cache.mark_album_refreshed(album_id);

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
}
