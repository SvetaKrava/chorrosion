// SPDX-License-Identifier: GPL-3.0-or-later
pub mod job;
pub mod jobs;
pub mod registry;

use anyhow::Result;
use chorrosion_config::AppConfig;
use registry::JobRegistry;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;

use jobs::{BacklogSearchJob, HousekeepingJob, RefreshAlbumJob, RefreshArtistJob, RssSyncJob};

#[allow(dead_code)]
pub struct Scheduler {
    config: AppConfig,
    registry: Arc<JobRegistry>,
}

impl Scheduler {
    pub fn new(config: AppConfig) -> Self {
        let registry = Arc::new(JobRegistry::new(config.scheduler.max_concurrent_jobs));
        Self { config, registry }
    }

    /// Register all background jobs with their schedules
    pub async fn register_jobs(&self) {
        info!(target: "scheduler", "registering background jobs");

        // RSS sync every 15 minutes
        self.registry
            .register("rss-sync", RssSyncJob::new(), Schedule::Interval(15 * 60))
            .await;

        // Backlog search every hour
        self.registry
            .register(
                "backlog-search",
                BacklogSearchJob::new(),
                Schedule::Interval(60 * 60),
            )
            .await;

        // Refresh all artists metadata every 12 hours
        self.registry
            .register(
                "refresh-artists",
                RefreshArtistJob::all(),
                Schedule::Interval(12 * 60 * 60),
            )
            .await;

        // Refresh all albums metadata every 12 hours (staggered after artists)
        self.registry
            .register(
                "refresh-albums",
                RefreshAlbumJob::all(),
                Schedule::Interval(12 * 60 * 60),
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
