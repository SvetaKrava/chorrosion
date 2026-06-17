// SPDX-License-Identifier: GPL-3.0-or-later
pub mod job;
pub mod jobs;
pub mod registry;

use anyhow::Result;
use chorrosion_config::AppConfig;
use chorrosion_infrastructure::sqlite_adapters::{
    SqliteAlbumRepository, SqliteDownloadClientDefinitionRepository,
    SqliteIndexerDefinitionRepository,
};
use chorrosion_musicbrainz::MusicBrainzClient;
use registry::JobRegistry;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;

use jobs::{
    BacklogSearchJob, DiscogsMetadataRefreshJob, HousekeepingJob, LastFmMetadataRefreshJob,
    RefreshAlbumJob, RefreshArtistJob, RssSyncJob,
};

#[allow(dead_code)]
pub struct Scheduler {
    config: AppConfig,
    registry: Arc<JobRegistry>,
    pool: SqlitePool,
}

impl Scheduler {
    pub fn new(config: AppConfig, pool: SqlitePool) -> Self {
        let registry = Arc::new(JobRegistry::new(config.scheduler.max_concurrent_jobs));
        Self {
            config,
            registry,
            pool,
        }
    }

    /// Register all background jobs with their schedules
    pub async fn register_jobs(&self) {
        info!(target: "scheduler", "registering background jobs");

        // RSS sync every 15 minutes
        let rss_album_repository = Arc::new(SqliteAlbumRepository::new_with_threshold(
            self.pool.clone(),
            self.config.database.slow_query_threshold_ms,
        ));
        let rss_indexer_repository =
            Arc::new(SqliteIndexerDefinitionRepository::new(self.pool.clone()));
        let rss_download_client_repository = Arc::new(
            SqliteDownloadClientDefinitionRepository::new(self.pool.clone()),
        );
        self.registry
            .register(
                "rss-sync",
                RssSyncJob::new(
                    rss_album_repository,
                    rss_indexer_repository,
                    rss_download_client_repository,
                ),
                Schedule::Interval(15 * 60),
            )
            .await;

        // Backlog search every hour, reusing the caller-provided database pool
        let album_repository = Arc::new(SqliteAlbumRepository::new_with_threshold(
            self.pool.clone(),
            self.config.database.slow_query_threshold_ms,
        ));
        self.registry
            .register(
                "backlog-search",
                BacklogSearchJob::new(album_repository),
                Schedule::Interval(60 * 60),
            )
            .await;

        // Refresh all artists metadata every 12 hours
        let mb_client_artists = Arc::new(match MusicBrainzClient::new() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(target: "scheduler", error = %e, "failed to create MusicBrainz client for artist refresh; job will be skipped");
                return;
            }
        });
        let refresh_artist_cache = jobs::MetadataRefreshCache::new();
        self.registry
            .register(
                "refresh-artists",
                RefreshArtistJob::with_dependencies(
                    None,
                    self.pool.clone(),
                    mb_client_artists,
                    refresh_artist_cache,
                ),
                Schedule::Interval(12 * 60 * 60),
            )
            .await;

        // Refresh all albums metadata every 12 hours, offset by 15 minutes from artists
        let mb_client_albums = Arc::new(match MusicBrainzClient::new() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(target: "scheduler", error = %e, "failed to create MusicBrainz client for album refresh; job will be skipped");
                return;
            }
        });
        let refresh_album_cache = jobs::MetadataRefreshCache::new();
        self.registry
            .register(
                "refresh-albums",
                RefreshAlbumJob::with_dependencies(
                    None,
                    self.pool.clone(),
                    mb_client_albums,
                    refresh_album_cache,
                ),
                Schedule::Interval(12 * 60 * 60 + 15 * 60),
            )
            .await;

        // Housekeeping every 24 hours
        self.registry
            .register(
                "housekeeping",
                HousekeepingJob::new(),
                Schedule::Interval(24 * 60 * 60),
            )
            .await;

        match LastFmMetadataRefreshJob::from_config_with_cache(
            &self.config.metadata.lastfm,
            &self.config.cache,
        ) {
            Some(lastfm_job) => {
                self.registry
                    .register(
                        "lastfm-metadata-refresh",
                        lastfm_job,
                        Schedule::Interval(6 * 60 * 60),
                    )
                    .await;
                info!(target: "scheduler", "Last.fm metadata refresh job registered");
            }
            None => {
                info!(target: "scheduler", "Last.fm metadata refresh job skipped (no API key configured)");
            }
        }

        match DiscogsMetadataRefreshJob::from_config_with_cache(
            &self.config.metadata.discogs,
            &self.config.cache,
        ) {
            Some(discogs_job) => {
                self.registry
                    .register(
                        "discogs-metadata-refresh",
                        discogs_job,
                        Schedule::Interval(6 * 60 * 60 + 30 * 60),
                    )
                    .await;
                info!(target: "scheduler", "Discogs metadata refresh job registered");
            }
            None => {
                info!(target: "scheduler", "Discogs metadata refresh job skipped (no seeds configured)");
            }
        }

        info!(target: "scheduler", "all jobs registered");
    }

    /// Start the scheduler and return a handle to the background task
    pub fn start(self) -> JoinHandle<Result<()>> {
        let registry = self.registry.clone();
        tokio::spawn(async move {
            registry.start().await;
            Ok(())
        })
    }
}

// Re-export key types for convenience
pub use job::{Job, JobContext, JobResult};
pub use registry::Schedule;
