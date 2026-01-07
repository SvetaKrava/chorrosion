// SPDX-License-Identifier: GPL-3.0-or-later
use crate::job::{Job, JobContext, JobResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

/// Job schedule configuration
#[derive(Debug, Clone)]
pub enum Schedule {
    /// Run at fixed intervals (in seconds)
    Interval(u64),
    /// Run once immediately, then never again
    Once,
    /// Cron-like schedule (future enhancement)
    Cron(String),
}

/// Registered job with its schedule
struct RegisteredJob {
    job: Arc<dyn Job>,
    schedule: Schedule,
}

/// Job registry that manages and executes scheduled jobs
pub struct JobRegistry {
    jobs: Arc<RwLock<HashMap<String, RegisteredJob>>>,
    max_concurrent: usize,
}

impl JobRegistry {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            max_concurrent,
        }
    }

    /// Register a job with its schedule
    pub async fn register(
        &self,
        job_id: impl Into<String>,
        job: impl Job + 'static,
        schedule: Schedule,
    ) {
        let job_id = job_id.into();
        let registered = RegisteredJob {
            job: Arc::new(job) as Arc<dyn Job>,
            schedule,
        };

        let mut jobs = self.jobs.write().await;
        info!(target: "registry", %job_id, job_type = registered.job.job_type(), "registering job");
        jobs.insert(job_id, registered);
    }

    /// Start the job registry executor
    pub async fn start(self: Arc<Self>) {
        info!(target: "registry", max_concurrent = self.max_concurrent, "starting job registry");

        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        let jobs = self.jobs.read().await;

        for (job_id, registered) in jobs.iter() {
            match &registered.schedule {
                Schedule::Interval(seconds) => {
                    let job_id = job_id.clone();
                    let job = registered.job.clone();
                    let interval_duration = Duration::from_secs(*seconds);
                    let semaphore = semaphore.clone();

                    tokio::spawn(async move {
                        let mut ticker = interval(interval_duration);
                        loop {
                            ticker.tick().await;
                            let permit = semaphore.clone().acquire_owned().await;
                            if let Ok(permit) = permit {
                                let job = job.clone();
                                let job_id = job_id.clone();
                                tokio::spawn(async move {
                                    let _permit = permit;
                                    Self::execute_job(job_id, job).await;
                                });
                            }
                        }
                    });
                }
                Schedule::Once => {
                    let job_id = job_id.clone();
                    let job = registered.job.clone();
                    let semaphore = semaphore.clone();

                    tokio::spawn(async move {
                        let permit = semaphore.acquire_owned().await;
                        if let Ok(_permit) = permit {
                            Self::execute_job(job_id, job).await;
                        }
                    });
                }
                Schedule::Cron(_expr) => {
                    warn!(target: "registry", %job_id, "cron schedules not yet implemented, skipping");
                }
            }
        }

        info!(target: "registry", "job registry started with {} jobs", jobs.len());
    }

    /// Execute a single job with retry logic
    async fn execute_job(job_id: String, job: Arc<dyn Job>) {
        let ctx = JobContext::new(&job_id);
        let mut attempts = 0;
        let max_attempts = if job.is_retriable() {
            job.max_retries() + 1
        } else {
            1
        };

        loop {
            attempts += 1;
            info!(
                target: "registry",
                job_id = %job_id,
                job_type = job.job_type(),
                attempt = attempts,
                max_attempts,
                "executing job"
            );

            match job.execute(ctx.clone()).await {
                Ok(JobResult::Success) => {
                    info!(
                        target: "registry",
                        job_id = %job_id,
                        job_type = job.job_type(),
                        attempts,
                        "job completed successfully"
                    );
                    break;
                }
                Ok(JobResult::Failure { error, retry }) => {
                    error!(
                        target: "registry",
                        job_id = %job_id,
                        job_type = job.job_type(),
                        attempts,
                        %error,
                        retry,
                        "job failed"
                    );

                    if retry && attempts < max_attempts {
                        let delay = Duration::from_secs(job.retry_delay_seconds());
                        warn!(
                            target: "registry",
                            job_id = %job_id,
                            ?delay,
                            "retrying job after delay"
                        );
                        tokio::time::sleep(delay).await;
                    } else {
                        error!(
                            target: "registry",
                            job_id = %job_id,
                            "job exhausted all retry attempts"
                        );
                        break;
                    }
                }
                Err(err) => {
                    error!(
                        target: "registry",
                        job_id = %job_id,
                        job_type = job.job_type(),
                        attempts,
                        error = %err,
                        "job execution error"
                    );

                    if job.is_retriable() && attempts < max_attempts {
                        let delay = Duration::from_secs(job.retry_delay_seconds());
                        warn!(
                            target: "registry",
                            job_id = %job_id,
                            ?delay,
                            "retrying job after delay"
                        );
                        tokio::time::sleep(delay).await;
                    } else {
                        break;
                    }
                }
            }
        }
    }
}
