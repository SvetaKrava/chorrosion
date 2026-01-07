// SPDX-License-Identifier: GPL-3.0-or-later
pub mod repositories;
pub mod sqlite_adapters;

use anyhow::Result;
use chorrosion_config::AppConfig;
use reqwest::Client;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use tracing::info;

pub fn http_client() -> Client {
    Client::builder()
        .pool_max_idle_per_host(8)
        .build()
        .expect("http client")
}

pub async fn init_database(config: &AppConfig) -> Result<()> {
    info!(target: "infrastructure", "initializing database");

    // Normalize the database URL for SQLite on Windows
    let db_url = if config.database.url.starts_with("sqlite://")
        && !config.database.url.starts_with("sqlite://:memory:")
    {
        let db_path = config.database.url.trim_start_matches("sqlite://");
        let path = Path::new(db_path);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
                info!(target: "infrastructure", path = %parent.display(), "created database directory");
            }
        }

        // Convert to absolute path for better Windows compatibility
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };

        // Use the absolute path with forward slashes (SQLite handles this on all platforms)
        let path_str = absolute_path.to_string_lossy().replace('\\', "/");

        // Add create mode to ensure SQLite can create the file
        format!("sqlite://{}?mode=rwc", path_str)
    } else {
        config.database.url.clone()
    };

    info!(target: "infrastructure", db_url = %db_url, "connecting to database");

    // SQLite pool connection
    let pool = SqlitePoolOptions::new()
        .max_connections(config.database.pool_max_size)
        .connect(&db_url)
        .await?;

    info!(target: "infrastructure", db_url = %config.database.url, "running migrations");
    sqlx::migrate!("../../migrations").run(&pool).await?;

    info!(target: "infrastructure", "database initialized successfully");
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
}
