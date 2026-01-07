// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::fmt;

/// Represents the execution context for a job
#[derive(Clone)]
pub struct JobContext {
    pub job_id: String,
    pub execution_time: DateTime<Utc>,
}

impl JobContext {
    pub fn new(job_id: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
            execution_time: Utc::now(),
        }
    }
}

/// Job execution result with optional retry information
#[derive(Debug)]
pub enum JobResult {
    Success,
    Failure { error: String, retry: bool },
}

/// Core trait for all background jobs
#[async_trait::async_trait]
pub trait Job: Send + Sync {
    /// Unique identifier for this job type
    fn job_type(&self) -> &'static str;

    /// Human-readable job name
    fn name(&self) -> String;

    /// Execute the job with given context
    async fn execute(&self, ctx: JobContext) -> Result<JobResult>;

    /// Whether this job can be retried on failure
    fn is_retriable(&self) -> bool {
        true
    }

    /// Maximum number of retry attempts
    fn max_retries(&self) -> u32 {
        3
    }

    /// Backoff delay in seconds between retries
    fn retry_delay_seconds(&self) -> u64 {
        60
    }
}

impl fmt::Debug for dyn Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Job")
            .field("type", &self.job_type())
            .field("name", &self.name())
            .finish()
    }
}
