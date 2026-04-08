// SPDX-License-Identifier: GPL-3.0-or-later

//! Transactional helpers for atomic multi-step database operations.

use anyhow::Result;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::future::Future;
use std::pin::Pin;
use tracing::debug;

type TxFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Run an async closure atomically within a SQLite transaction.
///
/// SQL statements inside the closure should be executed against `&mut **tx`.
///
/// - On success: the transaction is committed and the closure value is returned.
/// - On error: the transaction is rolled back and the original error is returned.
pub async fn run_in_transaction<F, T>(pool: &SqlitePool, f: F) -> Result<T>
where
    F: for<'a> FnOnce(&'a mut Transaction<'_, Sqlite>) -> TxFuture<'a, T>,
{
    let mut tx = pool.begin().await?;
    debug!(target: "infrastructure", "transaction started");

    let operation_result = f(&mut tx).await;
    match operation_result {
        Ok(value) => {
            tx.commit().await?;
            debug!(target: "infrastructure", "transaction committed");
            Ok(value)
        }
        Err(err) => {
            match tx.rollback().await {
                Err(rollback_err) => {
                    tracing::warn!(
                        target: "infrastructure",
                        error = %rollback_err,
                        "transaction rollback failed; connection may have been dropped"
                    );
                }
                Ok(()) => {
                    debug!(target: "infrastructure", "transaction rolled back");
                }
            }
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_database;
    use chorrosion_config::AppConfig;

    async fn in_memory_pool() -> SqlitePool {
        let mut config = AppConfig::default();
        config.database.url = "sqlite://:memory:".to_string();
        config.database.pool_max_size = 1;
        init_database(&config)
            .await
            .expect("init_database should succeed")
    }

    #[tokio::test]
    async fn test_transaction_commits_on_success() {
        let pool = in_memory_pool().await;

        run_in_transaction(&pool, |tx| {
            Box::pin(async move {
                sqlx::query(
                    "INSERT INTO artists (id, name, status, monitored) VALUES (?, ?, ?, ?)",
                )
                .bind("artist-tx-1")
                .bind("TX Artist One")
                .bind("continuing")
                .bind(true)
                .execute(&mut **tx)
                .await?;

                sqlx::query(
                    "INSERT INTO artists (id, name, status, monitored) VALUES (?, ?, ?, ?)",
                )
                .bind("artist-tx-2")
                .bind("TX Artist Two")
                .bind("continuing")
                .bind(true)
                .execute(&mut **tx)
                .await?;

                Ok(())
            })
        })
        .await
        .expect("transaction should commit successfully");

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM artists WHERE id IN ('artist-tx-1', 'artist-tx-2')",
        )
        .fetch_one(&pool)
        .await
        .expect("count query should succeed");

        assert_eq!(count, 2, "both artists should be visible after commit");
    }

    #[tokio::test]
    async fn test_transaction_rolls_back_on_error() {
        let pool = in_memory_pool().await;

        let result: Result<()> = run_in_transaction(&pool, |tx| {
            Box::pin(async move {
                sqlx::query(
                    "INSERT INTO artists (id, name, status, monitored) VALUES (?, ?, ?, ?)",
                )
                .bind("artist-rollback-1")
                .bind("Should Be Rolled Back")
                .bind("continuing")
                .bind(true)
                .execute(&mut **tx)
                .await?;

                // Duplicate primary key -- UNIQUE violation triggers rollback.
                sqlx::query(
                    "INSERT INTO artists (id, name, status, monitored) VALUES (?, ?, ?, ?)",
                )
                .bind("artist-rollback-1")
                .bind("Duplicate")
                .bind("continuing")
                .bind(true)
                .execute(&mut **tx)
                .await?;

                Ok(())
            })
        })
        .await;

        assert!(
            result.is_err(),
            "transaction should have failed with UNIQUE violation"
        );

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM artists WHERE id = 'artist-rollback-1'")
                .fetch_one(&pool)
                .await
                .expect("count query should succeed");

        assert_eq!(count, 0, "rolled-back row must not be visible");
    }

    #[tokio::test]
    async fn test_transaction_returns_closure_value() {
        let pool = in_memory_pool().await;

        let inserted_id: String = run_in_transaction(&pool, |tx| {
            Box::pin(async move {
                let id = "artist-return-val-1".to_string();
                sqlx::query(
                    "INSERT INTO artists (id, name, status, monitored) VALUES (?, ?, ?, ?)",
                )
                .bind(&id)
                .bind("Return Value Artist")
                .bind("continuing")
                .bind(true)
                .execute(&mut **tx)
                .await?;
                Ok(id)
            })
        })
        .await
        .expect("transaction should succeed");

        assert_eq!(
            inserted_id, "artist-return-val-1",
            "closure value should be propagated"
        );
    }

    #[tokio::test]
    async fn test_partial_writes_not_visible_after_rollback() {
        let pool = in_memory_pool().await;

        sqlx::query("INSERT INTO artists (id, name, status, monitored) VALUES (?, ?, ?, ?)")
            .bind("artist-pre-existing")
            .bind("Pre-existing")
            .bind("continuing")
            .bind(true)
            .execute(&pool)
            .await
            .expect("baseline insert should succeed");

        let _: Result<()> = run_in_transaction(&pool, |tx| {
            Box::pin(async move {
                sqlx::query(
                    "INSERT INTO artists (id, name, status, monitored) VALUES (?, ?, ?, ?)",
                )
                .bind("artist-partial-1")
                .bind("Partial Artist")
                .bind("continuing")
                .bind(true)
                .execute(&mut **tx)
                .await?;

                // Invalid status -- trigger-based constraint will reject this.
                sqlx::query(
                    "INSERT INTO artists (id, name, status, monitored) VALUES (?, ?, ?, ?)",
                )
                .bind("artist-partial-2")
                .bind("Bad Status Artist")
                .bind("invalid-status")
                .bind(true)
                .execute(&mut **tx)
                .await?;

                Ok(())
            })
        })
        .await;

        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists")
            .fetch_one(&pool)
            .await
            .expect("count query should succeed");

        assert_eq!(
            total, 1,
            "only the pre-existing artist should remain; the partial transaction must be fully rolled back"
        );
    }
}
