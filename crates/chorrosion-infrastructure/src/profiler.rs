// SPDX-License-Identifier: GPL-3.0-or-later
//! Query profiling and diagnostics for the infrastructure layer.
//!
//! [`QueryProfiler`] wraps query closures with elapsed-time measurement and
//! logs a `WARN` trace event for any query that exceeds the configured
//! threshold.  It also exposes [`QueryProfiler::explain_query_plan`] which
//! runs SQLite's `EXPLAIN QUERY PLAN` and returns the output as plain strings
//! — useful for verifying that indexes are being used.
//!
//! # Configuration
//!
//! The slow-query threshold comes from `DatabaseConfig::slow_query_threshold_ms`
//! (default 50 ms).  Set it to `0` to disable slow-query warnings entirely.
//!
//! # Example
//!
//! ```rust,ignore
//! let profiler = QueryProfiler::new(pool.clone(), config.database.slow_query_threshold_ms);
//!
//! let rows = profiler
//!     .timed("artists::list", || async {
//!         sqlx::query("SELECT * FROM artists ORDER BY name LIMIT 100")
//!             .fetch_all(&pool)
//!             .await
//!     })
//!     .await?;
//!
//! let plan = profiler.explain_query_plan("SELECT * FROM artists WHERE monitored = 1").await?;
//! for line in plan {
//!     tracing::debug!(target: "profiler", "{}", line);
//! }
//! ```

use anyhow::Result;
use sqlx::SqlitePool;
use std::future::Future;
use std::time::Instant;
use tracing::{debug, warn};

/// Wraps a [`SqlitePool`] with query timing and diagnostic helpers.
#[derive(Clone)]
pub struct QueryProfiler {
    pool: SqlitePool,
    /// Queries slower than this are logged at WARN.  `0` disables the check.
    threshold_ms: u64,
}

impl QueryProfiler {
    /// Create a new profiler.
    ///
    /// * `pool` — the pool used for `explain_query_plan` queries.
    /// * `threshold_ms` — slow-query warning threshold in milliseconds (`0` disables).
    pub fn new(pool: SqlitePool, threshold_ms: u64) -> Self {
        Self { pool, threshold_ms }
    }

    /// Time a query future identified by `label`.
    ///
    /// A `DEBUG` event is emitted on the `profiler` target when DEBUG logging
    /// is enabled for that target.  If the elapsed time also exceeds
    /// `threshold_ms` (and `threshold_ms > 0`), an additional `WARN` event is
    /// emitted.
    pub async fn timed<F, Fut, T>(&self, label: &str, query_fn: F) -> Fut::Output
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let start = Instant::now();
        let result = query_fn().await;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        debug!(target: "profiler", label, elapsed_ms, "query completed");

        if self.threshold_ms > 0 && elapsed_ms >= self.threshold_ms {
            warn!(
                target: "profiler",
                label,
                elapsed_ms,
                threshold_ms = self.threshold_ms,
                "slow query detected"
            );
        }

        result
    }

    /// Run `EXPLAIN QUERY PLAN` for `sql` and return each output row as a
    /// `String` in the form `"<id>|<parent>|<notused>|<detail>"`.
    ///
    /// This is a diagnostic helper intended for development / admin tooling.
    /// It does **not** execute `sql` — only its query plan.
    pub async fn explain_query_plan(&self, sql: &str) -> Result<Vec<String>> {
        let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let rows = sqlx::query(&explain_sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("EXPLAIN QUERY PLAN failed: {e}"))?;

        let mut lines = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;
            // EXPLAIN QUERY PLAN returns: id INTEGER, parent INTEGER, notused INTEGER, detail TEXT
            let id: i64 = row
                .try_get("id")
                .map_err(|e| anyhow::anyhow!("EXPLAIN QUERY PLAN: failed to read 'id': {e}"))?;
            let parent: i64 = row
                .try_get("parent")
                .map_err(|e| anyhow::anyhow!("EXPLAIN QUERY PLAN: failed to read 'parent': {e}"))?;
            let notused: i64 = row.try_get("notused").map_err(|e| {
                anyhow::anyhow!("EXPLAIN QUERY PLAN: failed to read 'notused': {e}")
            })?;
            let detail: String = row
                .try_get("detail")
                .map_err(|e| anyhow::anyhow!("EXPLAIN QUERY PLAN: failed to read 'detail': {e}"))?;
            lines.push(format!("{id}|{parent}|{notused}|{detail}"));
        }
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory pool")
    }

    // -----------------------------------------------------------------------
    // timed() — threshold behaviour
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn timed_completes_and_returns_value() {
        let pool = test_pool().await;
        let profiler = QueryProfiler::new(pool.clone(), 50);

        let result: Result<i64, sqlx::Error> = profiler
            .timed("test::simple_select", || async {
                let row = sqlx::query("SELECT 42 AS val")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                use sqlx::Row;
                Ok::<i64, sqlx::Error>(row.get("val"))
            })
            .await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn timed_zero_threshold_still_returns_value() {
        // A threshold of 0 disables slow-query logging; the function still
        // returns the inner result correctly.
        let pool = test_pool().await;
        let profiler = QueryProfiler::new(pool.clone(), 0);

        let result: Result<i64, sqlx::Error> = profiler
            .timed("test::zero_threshold", || async {
                let row = sqlx::query("SELECT 1 AS val")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                use sqlx::Row;
                Ok::<i64, sqlx::Error>(row.get("val"))
            })
            .await;

        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn timed_propagates_error() {
        let pool = test_pool().await;
        let profiler = QueryProfiler::new(pool.clone(), 50);

        let result: Result<(), anyhow::Error> = profiler
            .timed("test::bad_query", || async {
                sqlx::query("SELECT * FROM nonexistent_table_xyz")
                    .fetch_all(&pool)
                    .await
                    .map(|_| ())
                    .map_err(anyhow::Error::from)
            })
            .await;

        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // explain_query_plan()
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn explain_query_plan_returns_rows_for_valid_sql() {
        let pool = test_pool().await;
        // Create a minimal table so the planner has something to explain.
        sqlx::query("CREATE TABLE ex_test (id INTEGER PRIMARY KEY, val TEXT)")
            .execute(&pool)
            .await
            .unwrap();

        let profiler = QueryProfiler::new(pool.clone(), 50);
        let plan = profiler
            .explain_query_plan("SELECT * FROM ex_test WHERE id = 1")
            .await
            .unwrap();

        // SQLite always produces at least one EXPLAIN row.
        assert!(!plan.is_empty());
        // Each line is pipe-separated with 4 fields.
        for line in &plan {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            assert_eq!(
                parts.len(),
                4,
                "expected 4 pipe-separated fields, got: {line}"
            );
        }
    }

    #[tokio::test]
    async fn explain_query_plan_shows_index_usage() {
        let pool = test_pool().await;
        sqlx::query("CREATE TABLE idx_test (id INTEGER PRIMARY KEY, name TEXT, monitored INTEGER)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE INDEX idx_idx_test_monitored_name ON idx_test(monitored, name)")
            .execute(&pool)
            .await
            .unwrap();

        let profiler = QueryProfiler::new(pool.clone(), 50);
        let plan = profiler
            .explain_query_plan("SELECT * FROM idx_test WHERE monitored = 1 ORDER BY name")
            .await
            .unwrap();

        // At least one plan row should reference the index we created.
        let mentions_index = plan
            .iter()
            .any(|l| l.contains("idx_idx_test_monitored_name"));
        assert!(
            mentions_index,
            "expected plan to mention the index, got:\n{}",
            plan.join("\n")
        );
    }

    #[tokio::test]
    async fn explain_query_plan_errors_on_invalid_sql() {
        let pool = test_pool().await;
        let profiler = QueryProfiler::new(pool.clone(), 50);

        let result = profiler
            .explain_query_plan("TOTALLY NOT SQL ??? FROM nowhere")
            .await;

        assert!(result.is_err());
    }
}
