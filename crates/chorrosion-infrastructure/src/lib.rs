// SPDX-License-Identifier: GPL-3.0-or-later
pub mod backup_restore;
pub mod cache;
pub mod postgres_adapters;
pub mod profiler;
pub mod repositories;
pub mod sqlite_adapters;
pub mod transaction;

pub use backup_restore::{create_sqlite_backup, restore_sqlite_backup};
pub use cache::{CachedResponse, ResponseCache};
pub use profiler::QueryProfiler;
pub use transaction::run_in_transaction;

use anyhow::Result;
use chorrosion_config::AppConfig;
use reqwest::Client;
#[cfg(feature = "postgres")]
use sqlx::postgres::PgConnectOptions;
#[cfg(feature = "postgres")]
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;
#[cfg(feature = "postgres")]
use sqlx::PgPool;
use sqlx::SqlitePool;
use std::path::Path;
#[cfg(feature = "postgres")]
use std::str::FromStr;
#[cfg(feature = "postgres")]
use std::time::Duration;
use tracing::info;

pub fn http_client() -> Client {
    Client::builder()
        .pool_max_idle_per_host(8)
        .build()
        .expect("http client")
}

fn normalize_database_url(config: &AppConfig) -> Result<String> {
    let db_url = if config.database.url.starts_with("sqlite://")
        && !config.database.url.starts_with("sqlite://:memory:")
    {
        let db_path = config.database.url.trim_start_matches("sqlite://");
        let path = Path::new(db_path);

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
                info!(target: "infrastructure", path = %parent.display(), "created database directory");
            }
        }

        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };

        let path_str = absolute_path.to_string_lossy().replace('\\', "/");

        format!("sqlite://{}?mode=rwc", path_str)
    } else {
        config.database.url.clone()
    };

    Ok(db_url)
}

pub async fn create_sqlite_pool(config: &AppConfig) -> Result<SqlitePool> {
    let db_url = normalize_database_url(config)?;

    info!(target: "infrastructure", db_url = %db_url, "connecting to database");

    let pool = SqlitePoolOptions::new()
        .max_connections(config.database.pool_max_size)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA foreign_keys = ON")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&db_url)
        .await?;

    Ok(pool)
}

pub async fn init_database(config: &AppConfig) -> Result<SqlitePool> {
    info!(target: "infrastructure", "initializing database");

    let pool = create_sqlite_pool(config).await?;

    info!(target: "infrastructure", db_url = %config.database.url, "running migrations");
    sqlx::migrate!("../../migrations").run(&pool).await?;

    info!(target: "infrastructure", "database initialized successfully");
    Ok(pool)
}

#[cfg(feature = "postgres")]
pub async fn create_postgres_pool(config: &AppConfig) -> Result<PgPool> {
    let redacted_db_url = redact_postgres_url(&config.database.url);
    info!(target: "infrastructure", db_url = %redacted_db_url, "connecting to postgres database");

    let pool = PgPoolOptions::new()
        .max_connections(config.database.pool_max_size)
        .min_connections(config.database.pool_min_idle)
        .acquire_timeout(Duration::from_secs(config.database.pool_acquire_timeout_secs))
        .idle_timeout(Duration::from_secs(config.database.pool_idle_timeout_secs))
        .max_lifetime(Duration::from_secs(config.database.pool_max_lifetime_secs))
        .connect(&config.database.url)
        .await?;

    Ok(pool)
}

#[cfg(feature = "postgres")]
pub async fn init_postgres_database(config: &AppConfig) -> Result<PgPool> {
    info!(target: "infrastructure", "initializing postgres database");

    let pool = create_postgres_pool(config).await?;

    let redacted_db_url = redact_postgres_url(&config.database.url);
    info!(target: "infrastructure", db_url = %redacted_db_url, "running postgres migrations");
    run_postgres_migrations(&pool).await?;

    info!(target: "infrastructure", "postgres database initialized successfully");
    Ok(pool)
}

#[cfg(feature = "postgres")]
fn redact_postgres_url(db_url: &str) -> String {
    match PgConnectOptions::from_str(db_url) {
        Ok(options) => {
            let db_name = options.get_database().unwrap_or("<none>");
            format!(
                "postgres://{}:{}/{}",
                options.get_host(),
                options.get_port(),
                db_name
            )
        }
        Err(_) => "postgres://<redacted>".to_string(),
    }
}

#[cfg(feature = "postgres")]
async fn run_postgres_migrations(pool: &PgPool) -> Result<()> {
    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/postgres");

    if !migrations_path.exists() {
        return Err(anyhow::anyhow!(
            "postgres migrations directory not found at {}; use a Postgres-specific migration set instead of the shared SQLite migrations",
            migrations_path.display()
        ));
    }

    let migrator = sqlx::migrate::Migrator::new(migrations_path).await?;
    migrator.run(pool).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_conversion_windows_style() {
        // Test that Windows-style paths are converted correctly
        let path = Path::new("data\\chorrosion.db");
        let normalized = path.to_string_lossy().replace('\\', "/");
        assert!(normalized.contains("/") || !normalized.contains("\\"));
    }

    #[test]
    fn test_path_conversion_unix_style() {
        // Test that Unix-style paths remain unchanged
        let path = Path::new("data/chorrosion.db");
        let normalized = path.to_string_lossy().replace('\\', "/");
        assert_eq!(normalized, "data/chorrosion.db");
    }

    #[test]
    fn test_relative_to_absolute_conversion() {
        // Test that relative paths can be converted to absolute
        let relative_path = Path::new("data/chorrosion.db");
        let result = std::env::current_dir().unwrap().join(relative_path);
        assert!(result.is_absolute());
    }

    #[test]
    fn test_parent_directory_extraction() {
        // Test that we can extract parent directory correctly
        let path = Path::new("data/chorrosion.db");
        let parent = path.parent();
        assert!(parent.is_some());
        assert_eq!(parent.unwrap(), Path::new("data"));
    }

    #[tokio::test]
    async fn test_sqlite_foreign_keys_are_enabled_per_connection() {
        let mut config = AppConfig::default();
        config.database.url = "sqlite://:memory:".to_string();
        // Keep a single connection for deterministic in-memory behavior.
        config.database.pool_max_size = 1;

        let pool = init_database(&config)
            .await
            .expect("init_database should succeed");

        let foreign_keys_enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .expect("PRAGMA foreign_keys should be queryable");
        assert_eq!(foreign_keys_enabled, 1, "foreign_keys pragma should be ON");
    }

    #[tokio::test]
    async fn test_db_constraints_reject_invalid_status_and_fk_violations() {
        let mut config = AppConfig::default();
        config.database.url = "sqlite://:memory:".to_string();
        config.database.pool_max_size = 1;

        let pool = init_database(&config)
            .await
            .expect("init_database should succeed");

        let bad_artist =
            sqlx::query("INSERT INTO artists (id, name, status, monitored) VALUES (?, ?, ?, ?)")
                .bind("artist-1")
                .bind("Artist")
                .bind("not-a-real-status")
                .bind(true)
                .execute(&pool)
                .await;
        assert!(
            bad_artist.is_err(),
            "invalid artist status should be rejected"
        );

        let fk_violation = sqlx::query(
            "INSERT INTO albums (id, artist_id, title, status, monitored) VALUES (?, ?, ?, ?, ?)",
        )
        .bind("album-1")
        .bind("missing-artist")
        .bind("Album")
        .bind("wanted")
        .bind(true)
        .execute(&pool)
        .await;

        assert!(
            fk_violation.is_err(),
            "album insert with missing artist_id should fail FK checks"
        );
    }
}
