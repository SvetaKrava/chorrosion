// SPDX-License-Identifier: GPL-3.0-or-later
//! File replacement workflow for quality upgrades.
//!
//! When a better-quality audio file has been downloaded, this module performs
//! the atomic swap of the on-disk file and optionally backs up the original
//! before removing it.
//!
//! # Safety guarantees
//!
//! - For **in-place upgrades** (`existing_path == final_path`): the candidate
//!   is first staged to a temporary path adjacent to `final_path` (same
//!   directory, same filesystem).  The original is then backed up or deleted,
//!   and only after that succeeds is the staged file renamed into place.  If
//!   retiring the original fails the staged file is cleaned up and the original
//!   is left untouched.
//! - For **cross-path upgrades** (`existing_path != final_path`): the new file
//!   is moved/copied to `final_path` and then the original is retired.
//!
//! # Cross-platform notes
//!
//! All path operations go through [`std::path::Path`] / [`PathBuf`]; directory
//! separators are never hard-coded.

use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, instrument, warn};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the file replacement service.
#[derive(Debug, Clone, Default)]
pub struct FileReplacementConfig {
    /// When `true`, the existing file is moved to `backup_dir` before being
    /// replaced.  When `false`, the existing file is simply removed.
    pub backup_replaced: bool,

    /// Directory used for backups when `backup_replaced` is `true`.  The
    /// directory is created on first use if it does not already exist.
    ///
    /// Has no effect when `backup_replaced` is `false`.
    pub backup_dir: Option<PathBuf>,
}

// ============================================================================
// Errors
// ============================================================================

/// Errors that can occur during file replacement.
#[derive(Debug, Error)]
pub enum FileReplacementError {
    /// The new (source) file does not exist.
    #[error("source file not found: {0}")]
    SourceNotFound(PathBuf),

    /// The existing (destination) file does not exist.
    #[error("destination file not found: {0}")]
    DestinationNotFound(PathBuf),

    /// Backup is enabled but no backup directory has been configured.
    #[error("backup_replaced is enabled but no backup_dir is configured")]
    BackupDirNotConfigured,

    /// An I/O error occurred.
    #[error("I/O error during file replacement: {0}")]
    Io(#[from] std::io::Error),
}

// ============================================================================
// Outcome
// ============================================================================

/// The result of a successful file replacement.
#[derive(Debug, Clone)]
pub struct ReplacementOutcome {
    /// Path of the file that was written (the new quality file).
    pub final_path: PathBuf,

    /// Path the backup was written to, if backups are enabled.
    pub backed_up_to: Option<PathBuf>,
}

// ============================================================================
// Service
// ============================================================================

/// Service that performs the atomic file replacement for quality upgrades.
#[derive(Debug, Clone)]
pub struct FileReplacementService {
    config: FileReplacementConfig,
}

impl FileReplacementService {
    /// Create a new [`FileReplacementService`] with the given configuration.
    pub fn new(config: FileReplacementConfig) -> Self {
        Self { config }
    }

    /// Replace `existing_path` with `new_file_path`, placing the result at
    /// `final_path`.
    ///
    /// Typical usage for an in-place upgrade:
    /// ```text
    /// existing_path == final_path   (the library path)
    /// new_file_path                 (the freshly downloaded/decoded file)
    /// ```
    ///
    /// The method also handles the case where `final_path` differs from
    /// `existing_path` (e.g. the naming pattern changed).
    ///
    /// # Safety sequence for in-place upgrades
    ///
    /// When `existing_path == final_path` the original file must not be
    /// overwritten before a backup (if configured) can be taken.  The
    /// implementation therefore:
    ///
    /// 1. Stages `new_file_path` to a temporary path adjacent to `final_path`.
    /// 2. Retires (backs up or deletes) `existing_path`.
    /// 3. Moves the staged file to `final_path`.
    ///
    /// If retiring the existing file fails (e.g. backup is enabled but no
    /// directory is configured), the staged file is cleaned up and the original
    /// is left untouched.
    ///
    /// # Errors
    ///
    /// Returns [`FileReplacementError`] when any I/O step fails or when backup
    /// is requested but no directory is configured.
    #[instrument(target = "file_replacement", skip(self), fields(
        existing = %existing_path.display(),
        new_file = %new_file_path.display(),
        destination = %final_path.display(),
    ))]
    pub fn replace_file(
        &self,
        existing_path: &Path,
        new_file_path: &Path,
        final_path: &Path,
    ) -> Result<ReplacementOutcome, FileReplacementError> {
        // Validate inputs.
        if !new_file_path.exists() {
            return Err(FileReplacementError::SourceNotFound(
                new_file_path.to_path_buf(),
            ));
        }
        if !existing_path.exists() {
            return Err(FileReplacementError::DestinationNotFound(
                existing_path.to_path_buf(),
            ));
        }

        // Ensure the destination directory exists.
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let in_place = paths_are_same(existing_path, final_path);

        if in_place {
            // ── In-place upgrade ─────────────────────────────────────────────
            // existing_path and final_path are the same file.  We must not
            // overwrite it before backing it up, so we stage the candidate to a
            // temporary path first.
            let staged = staging_path(final_path);
            place_file(new_file_path, &staged)?;
            debug!(target: "file_replacement", staged = %staged.display(), "staged new file alongside destination");

            // Retire (backup/delete) the existing file.  If this fails, clean
            // up the staged file so we leave the original intact.
            let backed_up_to = match self.retire_existing(existing_path) {
                Ok(v) => v,
                Err(e) => {
                    if let Err(cleanup_err) = std::fs::remove_file(&staged) {
                        warn!(
                            target: "file_replacement",
                            staged = %staged.display(),
                            error = %cleanup_err,
                            "failed to clean up staged file after retire_existing error"
                        );
                    }
                    return Err(e);
                }
            };

            // Move the staged candidate into the final location.
            place_file(&staged, final_path)?;
            debug!(target: "file_replacement", "moved staged file to final path");

            Ok(ReplacementOutcome {
                final_path: final_path.to_path_buf(),
                backed_up_to,
            })
        } else {
            // ── Different-path upgrade ────────────────────────────────────────
            // Place the new file at final_path first (unless it is already
            // there), then retire the old file.
            if !paths_are_same(new_file_path, final_path) {
                place_file(new_file_path, final_path)?;
                debug!(target: "file_replacement", "placed new file at destination");
            } else {
                debug!(target: "file_replacement", "source is already at final path, skipping move");
            }

            let backed_up_to = self.retire_existing(existing_path)?;

            Ok(ReplacementOutcome {
                final_path: final_path.to_path_buf(),
                backed_up_to,
            })
        }
    }

    /// Move or delete the existing file according to the backup policy.
    fn retire_existing(
        &self,
        existing_path: &Path,
    ) -> Result<Option<PathBuf>, FileReplacementError> {
        if !self.config.backup_replaced {
            std::fs::remove_file(existing_path)?;
            debug!(target: "file_replacement", path = %existing_path.display(), "removed old file");
            return Ok(None);
        }

        let backup_dir = self
            .config
            .backup_dir
            .as_deref()
            .ok_or(FileReplacementError::BackupDirNotConfigured)?;

        std::fs::create_dir_all(backup_dir)?;

        // Build a unique backup filename: <stem>.<backup_ext> where backup_ext
        // is a timestamp + original extension.
        let file_name = existing_path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("track"));
        let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
        let backup_name = format!("{}.{}", file_name.to_string_lossy(), timestamp);
        let backup_path = backup_dir.join(&backup_name);

        place_file(existing_path, &backup_path)?;
        debug!(target: "file_replacement", from = %existing_path.display(), to = %backup_path.display(), "backed up old file");

        Ok(Some(backup_path))
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Returns a temporary staging path adjacent to `final_path`.
///
/// The staged file lives in the same directory so that the final rename is
/// always on the same filesystem (atomic on most platforms).
///
/// `file_name()` returns `None` only for a bare root path (e.g. `/`), which
/// is never a valid audio file target; the `"unnamed"` fallback is a safe
/// guard for that degenerate input.
fn staging_path(final_path: &Path) -> PathBuf {
    let file_name = final_path
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("unnamed"));
    let staged_name = format!(".{}.staging", file_name.to_string_lossy());
    final_path.with_file_name(staged_name)
}

/// Attempt an atomic rename; fall back to copy + delete on cross-device moves.
fn place_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            // On Linux/Windows, cross-device renames return EXDEV / ERROR_NOT_SAME_DEVICE.
            // Fall back to copy + remove.
            warn!(
                target: "file_replacement",
                src = %src.display(),
                dst = %dst.display(),
                error = %rename_err,
                "rename failed, falling back to copy"
            );
            std::fs::copy(src, dst)?;
            std::fs::remove_file(src)?;
            Ok(())
        }
    }
}

/// Returns `true` if `a` and `b` resolve to the same filesystem path.
///
/// Uses [`std::fs::canonicalize`] when both paths exist; falls back to a
/// lexical comparison when one or both do not yet exist (e.g. the destination
/// is a planned path).
fn paths_are_same(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_temp(dir: &tempfile::TempDir, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        // Create parent directories if they don't exist (for paths like "subdir/file.txt")
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path
    }

    fn read(path: &Path) -> Vec<u8> {
        std::fs::read(path).unwrap()
    }

    // ---- replace_file — basic swap ----

    #[test]
    fn replaces_existing_with_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let existing = write_temp(&dir, "track.flac", b"old content");
        let new_file = write_temp(&dir, "track_new.flac", b"new content");
        let final_path = dir.path().join("track.flac");

        let svc = FileReplacementService::new(FileReplacementConfig::default());
        let outcome = svc.replace_file(&existing, &new_file, &final_path).unwrap();

        assert_eq!(read(&outcome.final_path), b"new content");
        assert!(outcome.backed_up_to.is_none());
        // Old "new_file" temp should be gone
        assert!(!new_file.exists());
    }

    #[test]
    fn replaces_to_different_final_path() {
        let dir = tempfile::tempdir().unwrap();
        let existing = write_temp(&dir, "artist/old_track.flac", b"old");
        let new_file = write_temp(&dir, "downloaded.flac", b"better");
        let final_path = dir.path().join("artist").join("new_track.flac");

        let svc = FileReplacementService::new(FileReplacementConfig::default());
        let outcome = svc.replace_file(&existing, &new_file, &final_path).unwrap();

        assert_eq!(read(&outcome.final_path), b"better");
        assert!(!existing.exists(), "old file should be removed");
    }

    // ---- replace_file — backup enabled ----

    #[test]
    fn backs_up_old_file_when_configured() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        let existing = write_temp(&dir, "track.mp3", b"mp3 data");
        let new_file = write_temp(&dir, "track_new.flac", b"flac data");
        let final_path = dir.path().join("track.flac");

        let svc = FileReplacementService::new(FileReplacementConfig {
            backup_replaced: true,
            backup_dir: Some(backup_dir.clone()),
        });
        let outcome = svc.replace_file(&existing, &new_file, &final_path).unwrap();

        assert_eq!(read(&outcome.final_path), b"flac data");
        let backup_path = outcome
            .backed_up_to
            .expect("backup should have been created");
        assert!(backup_dir.exists());
        assert_eq!(read(&backup_path), b"mp3 data");
        assert!(!existing.exists());
    }

    #[test]
    fn in_place_upgrade_backs_up_original_and_places_new_content() {
        // existing_path == final_path (the library file is upgraded in-place).
        // The original must be backed up and final_path must contain the new
        // content — no data loss must occur.
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");

        // Both existing and final point at the same file.
        let existing = write_temp(&dir, "track.flac", b"old content");
        let new_file = write_temp(&dir, "track_new.flac", b"new content");
        let final_path = existing.clone(); // same path

        let svc = FileReplacementService::new(FileReplacementConfig {
            backup_replaced: true,
            backup_dir: Some(backup_dir.clone()),
        });
        let outcome = svc.replace_file(&existing, &new_file, &final_path).unwrap();

        // Final path must hold the new content.
        assert_eq!(read(&outcome.final_path), b"new content");

        // Original content must have been backed up.
        let backup_path = outcome
            .backed_up_to
            .expect("backup should have been created for in-place upgrade");
        assert!(backup_dir.exists());
        assert_eq!(read(&backup_path), b"old content");

        // The temporary staging file must not be left behind.
        let staged = staging_path(&final_path);
        assert!(!staged.exists(), "staging file should be cleaned up");

        // The candidate temp file should be gone (moved, not copied).
        assert!(!new_file.exists(), "new_file should have been moved");
    }

    #[test]
    fn in_place_upgrade_no_backup_deletes_original() {
        let dir = tempfile::tempdir().unwrap();
        let existing = write_temp(&dir, "track.flac", b"old content");
        let new_file = write_temp(&dir, "track_new.flac", b"new content");
        let final_path = existing.clone();

        let svc = FileReplacementService::new(FileReplacementConfig::default());
        let outcome = svc.replace_file(&existing, &new_file, &final_path).unwrap();

        assert_eq!(read(&outcome.final_path), b"new content");
        assert!(outcome.backed_up_to.is_none());
    }

    // ---- replace_file — error cases ----

    #[test]
    fn returns_error_when_source_missing() {
        let dir = tempfile::tempdir().unwrap();
        let existing = write_temp(&dir, "track.mp3", b"data");
        let new_file = dir.path().join("nonexistent.flac");
        let final_path = dir.path().join("track.flac");

        let svc = FileReplacementService::new(FileReplacementConfig::default());
        assert!(matches!(
            svc.replace_file(&existing, &new_file, &final_path),
            Err(FileReplacementError::SourceNotFound(_))
        ));
    }

    #[test]
    fn returns_error_when_destination_missing() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("nonexistent.mp3");
        let new_file = write_temp(&dir, "new.flac", b"data");
        let final_path = dir.path().join("track.flac");

        let svc = FileReplacementService::new(FileReplacementConfig::default());
        assert!(matches!(
            svc.replace_file(&existing, &new_file, &final_path),
            Err(FileReplacementError::DestinationNotFound(_))
        ));
    }

    #[test]
    fn returns_error_when_backup_enabled_but_no_dir() {
        let dir = tempfile::tempdir().unwrap();
        let existing = write_temp(&dir, "track.mp3", b"old");
        let new_file = write_temp(&dir, "track_new.flac", b"new");
        let final_path = dir.path().join("track.flac");

        let svc = FileReplacementService::new(FileReplacementConfig {
            backup_replaced: true,
            backup_dir: None,
        });
        assert!(matches!(
            svc.replace_file(&existing, &new_file, &final_path),
            Err(FileReplacementError::BackupDirNotConfigured)
        ));
    }

    #[test]
    fn in_place_retire_failure_cleans_up_staged_file() {
        // When retiring the existing file fails (backup enabled but no dir
        // configured), the staged file must be removed and the original must
        // remain intact.
        let dir = tempfile::tempdir().unwrap();
        let existing = write_temp(&dir, "track.flac", b"original content");
        let new_file = write_temp(&dir, "track_new.flac", b"new content");
        let final_path = existing.clone(); // in-place

        let svc = FileReplacementService::new(FileReplacementConfig {
            backup_replaced: true,
            backup_dir: None, // triggers BackupDirNotConfigured
        });
        let result = svc.replace_file(&existing, &new_file, &final_path);

        assert!(matches!(
            result,
            Err(FileReplacementError::BackupDirNotConfigured)
        ));

        // The staged file must have been cleaned up.
        let staged = staging_path(&final_path);
        assert!(
            !staged.exists(),
            "staged file should have been removed on error"
        );

        // The original file must still be intact.
        assert_eq!(read(&existing), b"original content");
    }
}
