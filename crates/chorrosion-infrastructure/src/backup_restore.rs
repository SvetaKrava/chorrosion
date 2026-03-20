// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{anyhow, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Create a timestamped backup copy of a SQLite database file.
///
/// `database_url` must be a file-backed SQLite URL (for example:
/// `sqlite://data/chorrosion.db` or `sqlite://C:/data/chorrosion.db?mode=rwc`).
/// In-memory URLs are rejected.
pub fn create_sqlite_backup(database_url: &str, backup_dir: &Path) -> Result<PathBuf> {
    let source = resolve_sqlite_file_path(database_url)?;

    if !source.exists() {
        return Err(anyhow!(
            "database file does not exist: {}",
            source.display()
        ));
    }

    fs::create_dir_all(backup_dir)?;

    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("chorrosion");
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("db");
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");

    let backup_path = backup_dir.join(format!("{stem}-{timestamp}.backup.{extension}"));

    fs::copy(&source, &backup_path)?;

    info!(
        target: "infrastructure",
        source = %source.display(),
        backup = %backup_path.display(),
        "created sqlite backup"
    );

    Ok(backup_path)
}

/// Restore a SQLite database file from a backup.
///
/// This performs a copy to a temporary path followed by rename to reduce the chance
/// of a partially-written destination file if restoration fails mid-copy.
pub fn restore_sqlite_backup(database_url: &str, backup_file: &Path) -> Result<()> {
    if !backup_file.exists() {
        return Err(anyhow!(
            "backup file does not exist: {}",
            backup_file.display()
        ));
    }

    let destination = resolve_sqlite_file_path(database_url)?;

    if let Some(parent) = destination.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let temp_destination = restore_temp_path_for(&destination);

    fs::copy(backup_file, &temp_destination)?;
    if destination.exists() {
        fs::remove_file(&destination)?;
    }
    fs::rename(&temp_destination, &destination)?;

    info!(
        target: "infrastructure",
        backup = %backup_file.display(),
        destination = %destination.display(),
        "restored sqlite backup"
    );

    Ok(())
}

fn resolve_sqlite_file_path(database_url: &str) -> Result<PathBuf> {
    if !database_url.starts_with("sqlite://") {
        return Err(anyhow!(
            "backup/restore currently supports only sqlite:// URLs: {}",
            database_url
        ));
    }

    if database_url.starts_with("sqlite://:memory:") {
        return Err(anyhow!(
            "backup/restore does not support in-memory sqlite database URLs"
        ));
    }

    let raw_path = database_url
        .trim_start_matches("sqlite://")
        .split('?')
        .next()
        .unwrap_or_default();

    if raw_path.is_empty() {
        return Err(anyhow!("sqlite database URL did not include a file path"));
    }

    let raw_path = Path::new(raw_path);
    let absolute_path = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(raw_path)
    };

    Ok(absolute_path)
}

fn restore_temp_path_for(destination: &Path) -> PathBuf {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("chorrosion.db");
    destination.with_file_name(format!("{file_name}.restore.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chorrosion-{prefix}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp test directory should be created");
        dir
    }

    #[test]
    fn test_create_backup_copies_database_file() {
        let temp_root = unique_temp_dir("backup-create");
        let db_path = temp_root.join("data").join("chorrosion.db");
        fs::create_dir_all(db_path.parent().expect("parent should exist"))
            .expect("data directory should be created");
        fs::write(&db_path, b"database-state-v1").expect("db file should be written");

        let backup_dir = temp_root.join("backups");
        let db_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));

        let backup_path =
            create_sqlite_backup(&db_url, &backup_dir).expect("backup should be created");

        assert!(backup_path.exists(), "backup file should exist");
        assert_eq!(
            backup_path.parent().expect("backup should have parent"),
            backup_dir
        );
        assert_eq!(
            fs::read(&backup_path).expect("backup should be readable"),
            b"database-state-v1"
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_restore_backup_replaces_database_file_contents() {
        let temp_root = unique_temp_dir("backup-restore");
        let db_path = temp_root.join("data").join("chorrosion.db");
        fs::create_dir_all(db_path.parent().expect("parent should exist"))
            .expect("data directory should be created");
        fs::write(&db_path, b"original-db-content").expect("db file should be written");

        let backup_dir = temp_root.join("backups");
        let db_url = format!(
            "sqlite://{}?mode=rwc",
            db_path.to_string_lossy().replace('\\', "/")
        );
        let backup_path =
            create_sqlite_backup(&db_url, &backup_dir).expect("backup should be created");

        fs::write(&db_path, b"mutated-db-content").expect("db mutation should succeed");
        restore_sqlite_backup(&db_url, &backup_path).expect("restore should succeed");

        assert_eq!(
            fs::read(&db_path).expect("restored db should be readable"),
            b"original-db-content"
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_backup_rejects_in_memory_database_url() {
        let backup_dir = unique_temp_dir("backup-memory");
        let result = create_sqlite_backup("sqlite://:memory:", &backup_dir);
        assert!(result.is_err(), "in-memory URL should not be supported");

        let _ = fs::remove_dir_all(&backup_dir);
    }
}
