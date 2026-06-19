#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Mock job for testing retryable failures
    struct RetryableFailureJob {
        attempt_count: Arc<AtomicU32>,
        fail_on_attempt: u32,
    }

    #[async_trait::async_trait]
    impl Job for RetryableFailureJob {
        fn job_type(&self) -> &'static str {
            "retryable-failure-test"
        }

        fn name(&self) -> String {
            "Retryable Failure Test".to_string()
        }

        async fn execute(&self, _ctx: JobContext) -> anyhow::Result<JobResult> {
            let attempt = self.attempt_count.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt < self.fail_on_attempt {
                Ok(JobResult::Failure {
                    error: format!("Transient failure on attempt {}", attempt),
                    retry: true,
                })
            } else {
                Ok(JobResult::Success)
            }
        }

        fn is_retriable(&self) -> bool {
            true
        }

        fn max_retries(&self) -> u32 {
            3
        }

        fn retry_delay_seconds(&self) -> u64 {
            1
        }
    }

    /// Mock job for testing terminal failures (non-retryable)
    struct TerminalFailureJob {
        attempt_count: Arc<AtomicU32>,
    }

    #[async_trait::async_trait]
    impl Job for TerminalFailureJob {
        fn job_type(&self) -> &'static str {
            "terminal-failure-test"
        }

        fn name(&self) -> String {
            "Terminal Failure Test".to_string()
        }

        async fn execute(&self, _ctx: JobContext) -> anyhow::Result<JobResult> {
            let _ = self.attempt_count.fetch_add(1, Ordering::SeqCst);
            Ok(JobResult::Failure {
                error: "Terminal failure - not retryable".to_string(),
                retry: false,
            })
        }

        fn is_retriable(&self) -> bool {
            false
        }

        fn max_retries(&self) -> u32 {
            0
        }

        fn retry_delay_seconds(&self) -> u64 {
            60
        }
    }

    /// Mock job for testing panic/error execution paths
    struct PanicJob {
        attempt_count: Arc<AtomicU32>,
        fail_on_attempt: u32,
    }

    #[async_trait::async_trait]
    impl Job for PanicJob {
        fn job_type(&self) -> &'static str {
            "panic-test"
        }

        fn name(&self) -> String {
            "Panic Test Job".to_string()
        }

        async fn execute(&self, _ctx: JobContext) -> anyhow::Result<JobResult> {
            let attempt = self.attempt_count.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt < self.fail_on_attempt {
                Err(anyhow::anyhow!("Transient error on attempt {}", attempt))
            } else {
                Ok(JobResult::Success)
            }
        }

        fn is_retriable(&self) -> bool {
            true
        }

        fn max_retries(&self) -> u32 {
            3
        }

        fn retry_delay_seconds(&self) -> u64 {
            1
        }
    }

    /// Mock job for testing non-retriable error execution
    struct NonRetriableErrorJob {
        attempt_count: Arc<AtomicU32>,
    }

    #[async_trait::async_trait]
    impl Job for NonRetriableErrorJob {
        fn job_type(&self) -> &'static str {
            "non-retriable-error-test"
        }

        fn name(&self) -> String {
            "Non-Retriable Error Test".to_string()
        }

        async fn execute(&self, _ctx: JobContext) -> anyhow::Result<JobResult> {
            let _ = self.attempt_count.fetch_add(1, Ordering::SeqCst);
            Err(anyhow::anyhow!("Non-retriable configuration error"))
        }

        fn is_retriable(&self) -> bool {
            false
        }

        fn max_retries(&self) -> u32 {
            0
        }

        fn retry_delay_seconds(&self) -> u64 {
            60
        }
    }

    #[tokio::test]
    async fn retryable_failure_succeeds_on_retry() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let job = Arc::new(RetryableFailureJob {
            attempt_count: attempt_count.clone(),
            fail_on_attempt: 2, // Fail on attempt 1, succeed on attempt 2
        });

        JobRegistry::execute_job("test-job".to_string(), job).await;

        // Should have attempted twice (failed once, then succeeded)
        assert_eq!(attempt_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn terminal_failure_stops_immediately() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let job = Arc::new(TerminalFailureJob {
            attempt_count: attempt_count.clone(),
        });

        JobRegistry::execute_job("test-job".to_string(), job).await;

        // Should have only attempted once (terminal failure)
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retryable_failures_exhaust_max_retries() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let job = Arc::new(RetryableFailureJob {
            attempt_count: attempt_count.clone(),
            fail_on_attempt: u32::MAX, // Always fail
        });

        JobRegistry::execute_job("test-job".to_string(), job).await;

        // Should have attempted max_retries() + 1 times
        assert_eq!(attempt_count.load(Ordering::SeqCst), 4); // 3 retries + 1 initial
    }

    #[tokio::test]
    async fn retriable_error_retries_until_success() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let job = Arc::new(PanicJob {
            attempt_count: attempt_count.clone(),
            fail_on_attempt: 3, // Fail on attempts 1 and 2, succeed on 3
        });

        JobRegistry::execute_job("test-job".to_string(), job).await;

        // Should have attempted 3 times (fail, fail, succeed)
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn non_retriable_error_stops_immediately() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let job = Arc::new(NonRetriableErrorJob {
            attempt_count: attempt_count.clone(),
        });

        JobRegistry::execute_job("test-job".to_string(), job).await;

        // Should have only attempted once
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn successful_job_completes_on_first_attempt() {
        struct SuccessJob;

        #[async_trait::async_trait]
        impl Job for SuccessJob {
            fn job_type(&self) -> &'static str {
                "success-test"
            }

            fn name(&self) -> String {
                "Success Test".to_string()
            }

            async fn execute(&self, _ctx: JobContext) -> anyhow::Result<JobResult> {
                Ok(JobResult::Success)
            }

            fn is_retriable(&self) -> bool {
                true
            }

            fn max_retries(&self) -> u32 {
                3
            }

            fn retry_delay_seconds(&self) -> u64 {
                60
            }
        }

        let job = Arc::new(SuccessJob);
        JobRegistry::execute_job("test-job".to_string(), job).await;
        // Success on first attempt - execution completes without error
    }

    #[tokio::test]
    async fn backoff_delay_is_applied_between_retries() {
        let start = Instant::now();
        let attempt_count = Arc::new(AtomicU32::new(0));
        let job = Arc::new(RetryableFailureJob {
            attempt_count: attempt_count.clone(),
            fail_on_attempt: 3, // Fail on attempts 1 and 2, succeed on 3
        });

        JobRegistry::execute_job("test-job".to_string(), job).await;

        let elapsed = start.elapsed();

        // With 1-second delays and 2 failures, should take at least 2 seconds
        assert!(elapsed.as_secs() >= 2, "Backoff delay not applied properly");
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn max_retries_setting_is_respected() {
        struct CustomMaxRetriesJob {
            attempt_count: Arc<AtomicU32>,
        }

        #[async_trait::async_trait]
        impl Job for CustomMaxRetriesJob {
            fn job_type(&self) -> &'static str {
                "custom-max-retries-test"
            }

            fn name(&self) -> String {
                "Custom Max Retries Test".to_string()
            }

            async fn execute(&self, _ctx: JobContext) -> anyhow::Result<JobResult> {
                let _ = self.attempt_count.fetch_add(1, Ordering::SeqCst);
                Ok(JobResult::Failure {
                    error: "Always fail".to_string(),
                    retry: true,
                })
            }

            fn is_retriable(&self) -> bool {
                true
            }

            fn max_retries(&self) -> u32 {
                2 // Custom limit: 2 retries (3 total attempts)
            }

            fn retry_delay_seconds(&self) -> u64 {
                0
            }
        }

        let attempt_count = Arc::new(AtomicU32::new(0));
        let job = Arc::new(CustomMaxRetriesJob {
            attempt_count: attempt_count.clone(),
        });

        JobRegistry::execute_job("test-job".to_string(), job).await;

        // Should respect custom max_retries (2) + 1 initial = 3
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::job::{Job, JobContext, JobResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
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

            let attempt_start = Instant::now();
            let execution_result = job.execute(ctx.clone()).await;
            match execution_result {
                Ok(JobResult::Success) => {
                    let elapsed_ms = attempt_start.elapsed().as_millis() as u64;
                    info!(
                        target: "registry",
                        job_id = %job_id,
                        job_type = job.job_type(),
                        attempt = attempts,
                        max_attempts,
                        elapsed_ms,
                        "job completed successfully"
                    );
                    break;
                }
                Ok(JobResult::Failure { error, retry }) => {
                    let elapsed_ms = attempt_start.elapsed().as_millis() as u64;
                    error!(
                        target: "registry",
                        job_id = %job_id,
                        job_type = job.job_type(),
                        attempt = attempts,
                        max_attempts,
                        elapsed_ms,
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
                    let elapsed_ms = attempt_start.elapsed().as_millis() as u64;
                    error!(
                        target: "registry",
                        job_id = %job_id,
                        job_type = job.job_type(),
                        attempt = attempts,
                        max_attempts,
                        elapsed_ms,
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
