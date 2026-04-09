// SPDX-License-Identifier: GPL-3.0-or-later
//! Permission handling for file operations.
//!
//! This module provides cross-platform file permission handling, including:
//! - Permission checks before file operations (read, write, execute)
//! - Permission preservation during file copy/move operations
//! - Permission setting with configurable default modes
//! - Cross-platform support (Windows ACLs, Unix chmod)

use std::fs::{self, Permissions};
use std::io;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, instrument};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for file permissions.
#[derive(Debug, Clone)]
pub struct PermissionConfig {
    /// Default permission mode for files (Unix: 0o644 = rw-r--r--, default).
    /// On Windows, Unix mode bits are not fully applied, but the write bit is
    /// interpreted to derive the readonly flag when permissions are set.
    pub file_mode: u32,

    /// Default permission mode for directories (Unix: 0o755 = rwxr-xr-x, default).
    /// On Windows, Unix mode bits are not fully applied, but the write bit is
    /// interpreted to derive the readonly flag when permissions are set.
    pub dir_mode: u32,

    /// Whether to preserve original file permissions during copy/move.
    /// If `false`, applies `file_mode` instead.
    pub preserve_permissions: bool,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            #[cfg(unix)]
            file_mode: 0o644,
            #[cfg(not(unix))]
            file_mode: 0o666,

            #[cfg(unix)]
            dir_mode: 0o755,
            #[cfg(not(unix))]
            dir_mode: 0o777,

            preserve_permissions: true,
        }
    }
}

// ============================================================================
// Errors
// ============================================================================

/// Errors related to file permissions.
#[derive(Debug, Error)]
pub enum PermissionError {
    /// File does not have read permission.
    #[error("file is not readable: {0}")]
    ReadDenied(String),

    /// File does not have write permission.
    #[error("file is not writable: {0}")]
    WriteDenied(String),

    /// Directory does not have execute permission (listing).
    #[error("directory is not accessible: {0}")]
    ExecuteDenied(String),

    /// Generic metadata access error.
    #[error("failed to read file permissions: {0}")]
    MetadataError(#[from] io::Error),

    /// Failed to set file permissions.
    #[error("failed to set file permissions: {0}")]
    PermissionSetError(#[source] io::Error),
}

// ============================================================================
// Permission checker
// ============================================================================

/// Utility for checking and managing file permissions.
pub struct PermissionChecker;

impl PermissionChecker {
    /// Check if a file is readable.
    #[instrument(skip_all, fields(path = ?path.as_ref()))]
    pub fn is_readable<P: AsRef<Path>>(path: P) -> Result<bool, PermissionError> {
        let path = path.as_ref();
        let metadata = fs::metadata(path)?;

        // Attempt the actual operation so the OS enforces all permission bits
        // (owner, group, other) and ACLs rather than inspecting mode bits alone.
        let access_result = if metadata.is_dir() {
            fs::read_dir(path).map(|_| ())
        } else {
            fs::File::open(path).map(|_| ())
        };

        match access_result {
            Ok(()) => {
                debug!(
                    "Readable check succeeded via {} attempt",
                    if metadata.is_dir() {
                        "read_dir"
                    } else {
                        "open"
                    }
                );
                Ok(true)
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                debug!(
                    "Readable check denied via {} attempt: {}",
                    if metadata.is_dir() {
                        "read_dir"
                    } else {
                        "open"
                    },
                    err
                );
                Ok(false)
            }
            Err(err) => Err(PermissionError::MetadataError(err)),
        }
    }

    /// Check if a file is writable.
    #[instrument(skip_all, fields(path = ?path.as_ref()))]
    pub fn is_writable<P: AsRef<Path>>(path: P) -> Result<bool, PermissionError> {
        let path = path.as_ref();
        let metadata = fs::metadata(path)?;

        if metadata.is_dir() {
            // For directories, checking write permission without side effects requires
            // platform-specific handling.
            #[cfg(unix)]
            {
                // On Unix, check owner/group/other write bits. Note: this does not
                // account for ACLs, but there is no portable side-effect-free
                // alternative without OS-specific APIs.
                let mode = metadata.permissions().mode();
                let is_writable = (mode & 0o222) != 0;
                debug!(
                    "Unix directory writable check: mode={:o}, writable={}",
                    mode, is_writable
                );
                return Ok(is_writable);
            }
            #[cfg(not(unix))]
            {
                // On Windows, let the OS enforce ACLs via the readonly flag.
                let is_writable = !metadata.permissions().readonly();
                debug!(
                    "Windows directory writable check: readonly={}, writable={}",
                    metadata.permissions().readonly(),
                    is_writable
                );
                return Ok(is_writable);
            }
        }

        // For regular files, attempt the actual write open so the OS enforces all
        // permission bits (owner, group, other) and ACLs.
        let access_result = fs::OpenOptions::new().write(true).open(path);

        match access_result {
            Ok(_) => {
                debug!("File writable check succeeded via open attempt");
                Ok(true)
            }
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                debug!("File writable check denied via open attempt: {}", err);
                Ok(false)
            }
            Err(err) => Err(PermissionError::MetadataError(err)),
        }
    }

    /// Check if a directory is accessible (has execute permission).
    #[instrument(skip_all, fields(path = ?path.as_ref()))]
    pub fn is_accessible<P: AsRef<Path>>(path: P) -> Result<bool, PermissionError> {
        let path = path.as_ref();
        let metadata = fs::metadata(path)?;

        if !metadata.is_dir() {
            return Err(PermissionError::ExecuteDenied(format!(
                "{} is not a directory",
                path.display()
            )));
        }

        #[cfg(unix)]
        {
            // On Unix, check all execute bits (owner, group, other).  Checking
            // the full 0o111 mask is more accurate than inspecting only the
            // owner bit (0o100), though it does not yet account for effective
            // UID/GID vs. the file's UID/GID. A portable ACL-aware check would
            // require libc::access(path, X_OK).
            let mode = metadata.permissions().mode();
            let is_accessible = (mode & 0o111) != 0;
            debug!(
                "Unix accessible check: mode={:o}, accessible={}",
                mode, is_accessible
            );
            Ok(is_accessible)
        }

        #[cfg(not(unix))]
        {
            // On Windows, attempt the actual read_dir operation so the OS
            // enforces ACLs rather than always returning true.
            let access_result = fs::read_dir(path).map(|_| ());
            match access_result {
                Ok(()) => {
                    debug!("Windows accessible check succeeded via read_dir attempt");
                    Ok(true)
                }
                Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
                    debug!(
                        "Windows accessible check denied via read_dir attempt: {}",
                        err
                    );
                    Ok(false)
                }
                Err(err) => Err(PermissionError::MetadataError(err)),
            }
        }
    }

    /// Verify that a file is readable, raising an error if not.
    #[instrument(skip_all, fields(path = ?path.as_ref()))]
    pub fn check_readable<P: AsRef<Path>>(path: P) -> Result<(), PermissionError> {
        let path = path.as_ref();
        if Self::is_readable(path)? {
            Ok(())
        } else {
            Err(PermissionError::ReadDenied(path.display().to_string()))
        }
    }

    /// Verify that a file is writable, raising an error if not.
    #[instrument(skip_all, fields(path = ?path.as_ref()))]
    pub fn check_writable<P: AsRef<Path>>(path: P) -> Result<(), PermissionError> {
        let path = path.as_ref();
        if Self::is_writable(path)? {
            Ok(())
        } else {
            Err(PermissionError::WriteDenied(path.display().to_string()))
        }
    }

    /// Verify that a directory is accessible, raising an error if not.
    #[instrument(skip_all, fields(path = ?path.as_ref()))]
    pub fn check_accessible<P: AsRef<Path>>(path: P) -> Result<(), PermissionError> {
        let path = path.as_ref();
        if Self::is_accessible(path)? {
            Ok(())
        } else {
            Err(PermissionError::ExecuteDenied(path.display().to_string()))
        }
    }
}

// ============================================================================
// Permission manager
// ============================================================================

/// Utility for setting and preserving file permissions.
pub struct PermissionManager;

impl PermissionManager {
    /// Get the current permissions of a file.
    #[instrument(skip_all, fields(path = ?path.as_ref()))]
    pub fn get_permissions<P: AsRef<Path>>(path: P) -> Result<Permissions, PermissionError> {
        let path = path.as_ref();
        fs::metadata(path)
            .map_err(PermissionError::from)
            .map(|m| m.permissions())
    }

    /// Set permissions on a file using the specified mode.
    ///
    /// On Unix systems, this applies the given mode via chmod.
    /// On Windows, the mode is interpreted as a readonly flag.
    #[instrument(skip_all, fields(path = ?path.as_ref()))]
    pub fn set_permissions<P: AsRef<Path>>(path: P, mode: u32) -> Result<(), PermissionError> {
        let path = path.as_ref();

        #[cfg(unix)]
        {
            let permissions = Permissions::from_mode(mode);
            fs::set_permissions(path, permissions).map_err(|e| {
                debug!(
                    "Failed to set Unix permissions {:o} on {}: {}",
                    mode,
                    path.display(),
                    e
                );
                PermissionError::PermissionSetError(e)
            })?;
            debug!("Set Unix permissions {:o} on {}", mode, path.display());
        }

        #[cfg(not(unix))]
        {
            let readonly = (mode & 0o200) == 0; // If write bit is not set, mark as readonly
            let mut permissions = fs::metadata(path)?.permissions();
            permissions.set_readonly(readonly);
            fs::set_permissions(path, permissions).map_err(|e| {
                debug!(
                    "Failed to set Windows permissions (readonly={}) on {}: {}",
                    readonly,
                    path.display(),
                    e
                );
                PermissionError::PermissionSetError(e)
            })?;
            debug!(
                "Set Windows permissions (readonly={}) on {}",
                readonly,
                path.display()
            );
        }

        Ok(())
    }

    /// Preserve permissions from source to destination.
    ///
    /// Copies the permissions from `src` and applies them to `dst`.
    #[instrument(skip_all, fields(src = ?src.as_ref(), dst = ?dst.as_ref()))]
    pub fn preserve_permissions<S: AsRef<Path>, D: AsRef<Path>>(
        src: S,
        dst: D,
    ) -> Result<(), PermissionError> {
        let src = src.as_ref();
        let dst = dst.as_ref();

        let src_metadata = fs::metadata(src)?;
        let src_permissions = src_metadata.permissions();

        fs::set_permissions(dst, src_permissions).map_err(|e| {
            debug!(
                "Failed to preserve permissions from {} to {}: {}",
                src.display(),
                dst.display(),
                e
            );
            PermissionError::PermissionSetError(e)
        })?;

        debug!(
            "Preserved permissions from {} to {}",
            src.display(),
            dst.display()
        );
        Ok(())
    }

    /// Apply default permissions from the configuration to a file.
    ///
    /// Determines whether the file is a regular file or directory and applies
    /// the corresponding default mode from the configuration.
    #[instrument(skip_all, fields(path = ?path.as_ref()))]
    pub fn apply_defaults<P: AsRef<Path>>(
        path: P,
        config: &PermissionConfig,
    ) -> Result<(), PermissionError> {
        let path = path.as_ref();
        let metadata = fs::metadata(path)?;

        let mode = if metadata.is_dir() {
            config.dir_mode
        } else {
            config.file_mode
        };

        Self::set_permissions(path, mode)
    }

    /// Copy permissions from source to destination based on configuration.
    ///
    /// If `preserve_permissions` is enabled, copies from source.
    /// Otherwise, applies default modes from configuration.
    #[instrument(skip_all, fields(src = ?src.as_ref(), dst = ?dst.as_ref()))]
    pub fn apply_permissions<S: AsRef<Path>, D: AsRef<Path>>(
        src: S,
        dst: D,
        config: &PermissionConfig,
    ) -> Result<(), PermissionError> {
        let src = src.as_ref();
        let dst = dst.as_ref();

        if config.preserve_permissions {
            Self::preserve_permissions(src, dst)
        } else {
            Self::apply_defaults(dst, config)
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_temp_file() -> io::Result<(NamedTempFile, std::path::PathBuf)> {
        let file = NamedTempFile::new()?;
        let path = file.path().to_path_buf();
        Ok((file, path))
    }

    #[test]
    fn test_is_readable_on_regular_file() -> io::Result<()> {
        let (_file, path) = create_temp_file()?;
        assert!(PermissionChecker::is_readable(&path).unwrap());
        Ok(())
    }

    #[test]
    fn test_is_writable_on_regular_file() -> io::Result<()> {
        let (_file, path) = create_temp_file()?;
        assert!(PermissionChecker::is_writable(&path).unwrap());
        Ok(())
    }

    #[test]
    fn test_is_accessible_on_directory() -> io::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        assert!(PermissionChecker::is_accessible(temp_dir.path()).unwrap());
        Ok(())
    }

    #[test]
    fn test_check_readable_succeeds_on_readable_file() -> io::Result<()> {
        let (_file, path) = create_temp_file()?;
        assert!(PermissionChecker::check_readable(&path).is_ok());
        Ok(())
    }

    #[test]
    fn test_check_writable_succeeds_on_writable_file() -> io::Result<()> {
        let (_file, path) = create_temp_file()?;
        assert!(PermissionChecker::check_writable(&path).is_ok());
        Ok(())
    }

    #[test]
    fn test_check_accessible_succeeds_on_accessible_directory() -> io::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        assert!(PermissionChecker::check_accessible(temp_dir.path()).is_ok());
        Ok(())
    }

    #[test]
    fn test_check_readable_fails_on_nonexistent_file() {
        let path = std::path::Path::new("/nonexistent/file/path");
        assert!(PermissionChecker::check_readable(path).is_err());
    }

    #[test]
    fn test_get_permissions_on_regular_file() -> io::Result<()> {
        let (_file, path) = create_temp_file()?;
        let perms = PermissionManager::get_permissions(&path).unwrap();
        assert!(!perms.readonly() || cfg!(unix));
        Ok(())
    }

    #[test]
    fn test_preserve_permissions_copies_from_source() -> io::Result<()> {
        let src_file = NamedTempFile::new()?;
        let src_path = src_file.path().to_path_buf();

        let dst_file = NamedTempFile::new()?;
        let dst_path = dst_file.path().to_path_buf();

        // Get original permissions
        let src_perms = fs::metadata(&src_path)?.permissions();

        // Preserve to destination
        PermissionManager::preserve_permissions(&src_path, &dst_path).unwrap();

        // Verify destination has same permissions
        let dst_perms = fs::metadata(&dst_path)?.permissions();
        #[cfg(unix)]
        assert_eq!(src_perms.mode(), dst_perms.mode());
        #[cfg(not(unix))]
        assert_eq!(src_perms.readonly(), dst_perms.readonly());

        Ok(())
    }

    #[test]
    fn test_apply_defaults_sets_file_mode() -> io::Result<()> {
        let (_file, path) = create_temp_file()?;
        let config = PermissionConfig::default();

        PermissionManager::apply_defaults(&path, &config).unwrap();

        #[cfg(unix)]
        {
            let perms = fs::metadata(&path)?.permissions();
            assert_eq!(perms.mode() & 0o777, config.file_mode);
        }

        Ok(())
    }

    #[test]
    fn test_apply_defaults_sets_directory_mode() -> io::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let config = PermissionConfig::default();

        PermissionManager::apply_defaults(temp_dir.path(), &config).unwrap();

        #[cfg(unix)]
        {
            let perms = fs::metadata(temp_dir.path())?.permissions();
            assert_eq!(perms.mode() & 0o777, config.dir_mode);
        }

        Ok(())
    }

    #[test]
    fn test_apply_permissions_preserves_when_enabled() -> io::Result<()> {
        let src_file = NamedTempFile::new()?;
        let src_path = src_file.path().to_path_buf();

        let dst_file = NamedTempFile::new()?;
        let dst_path = dst_file.path().to_path_buf();

        let mut config = PermissionConfig::default();
        config.preserve_permissions = true;

        PermissionManager::apply_permissions(&src_path, &dst_path, &config).unwrap();

        let src_perms = fs::metadata(&src_path)?.permissions();
        let dst_perms = fs::metadata(&dst_path)?.permissions();

        #[cfg(unix)]
        assert_eq!(src_perms.mode(), dst_perms.mode());
        #[cfg(not(unix))]
        assert_eq!(src_perms.readonly(), dst_perms.readonly());

        Ok(())
    }

    #[test]
    fn test_apply_permissions_applies_defaults_when_disabled() -> io::Result<()> {
        let src_file = NamedTempFile::new()?;
        let src_path = src_file.path().to_path_buf();

        let dst_file = NamedTempFile::new()?;
        let dst_path = dst_file.path().to_path_buf();

        let mut config = PermissionConfig::default();
        config.preserve_permissions = false;

        PermissionManager::apply_permissions(&src_path, &dst_path, &config).unwrap();

        #[cfg(unix)]
        {
            let dst_perms = fs::metadata(&dst_path)?.permissions();
            assert_eq!(dst_perms.mode() & 0o777, config.file_mode);
        }

        Ok(())
    }

    #[test]
    fn test_set_permissions_with_custom_mode() -> io::Result<()> {
        let (_file, path) = create_temp_file()?;

        #[cfg(unix)]
        let custom_mode = 0o755;
        #[cfg(not(unix))]
        let custom_mode = 0o666;

        PermissionManager::set_permissions(&path, custom_mode).unwrap();

        #[cfg(unix)]
        {
            let perms = fs::metadata(&path)?.permissions();
            assert_eq!(perms.mode() & 0o777, custom_mode);
        }

        Ok(())
    }

    #[test]
    fn test_permission_error_display() {
        let err = PermissionError::ReadDenied("/path/to/file".to_string());
        assert!(err.to_string().contains("not readable"));
    }

    #[test]
    fn test_permission_config_default() {
        let config = PermissionConfig::default();
        assert!(config.preserve_permissions);
        #[cfg(unix)]
        {
            assert_eq!(config.file_mode, 0o644);
            assert_eq!(config.dir_mode, 0o755);
        }
    }

    #[cfg(unix)]
    mod unix_permission_denied {
        use super::*;
        use tempfile::NamedTempFile;

        /// Set Unix mode bits on a path.
        fn chmod(path: &std::path::Path, mode: u32) {
            let perms = std::fs::Permissions::from_mode(mode);
            fs::set_permissions(path, perms).expect("chmod failed");
        }

        #[test]
        fn test_is_readable_returns_false_for_no_read_bit_file() -> io::Result<()> {
            let file = NamedTempFile::new()?;
            let path = file.path().to_path_buf();
            chmod(&path, 0o000);
            let result = PermissionChecker::is_readable(&path);
            // Restore before asserting so the file can be cleaned up
            chmod(&path, 0o644);
            assert!(!result.unwrap(), "expected not readable for mode 0o000");
            Ok(())
        }

        #[test]
        fn test_check_readable_returns_read_denied_for_no_read_bit_file() -> io::Result<()> {
            let file = NamedTempFile::new()?;
            let path = file.path().to_path_buf();
            chmod(&path, 0o000);
            let result = PermissionChecker::check_readable(&path);
            chmod(&path, 0o644);
            assert!(
                matches!(result, Err(PermissionError::ReadDenied(_))),
                "expected ReadDenied, got {:?}",
                result
            );
            Ok(())
        }

        #[test]
        fn test_is_writable_returns_false_for_no_write_bit_file() -> io::Result<()> {
            let file = NamedTempFile::new()?;
            let path = file.path().to_path_buf();
            chmod(&path, 0o444);
            let result = PermissionChecker::is_writable(&path);
            chmod(&path, 0o644);
            assert!(!result.unwrap(), "expected not writable for mode 0o444");
            Ok(())
        }

        #[test]
        fn test_check_writable_returns_write_denied_for_readonly_file() -> io::Result<()> {
            let file = NamedTempFile::new()?;
            let path = file.path().to_path_buf();
            chmod(&path, 0o444);
            let result = PermissionChecker::check_writable(&path);
            chmod(&path, 0o644);
            assert!(
                matches!(result, Err(PermissionError::WriteDenied(_))),
                "expected WriteDenied, got {:?}",
                result
            );
            Ok(())
        }

        #[test]
        fn test_is_accessible_returns_false_for_no_execute_bit_dir() -> io::Result<()> {
            let dir = tempfile::tempdir()?;
            chmod(dir.path(), 0o600);
            let result = PermissionChecker::is_accessible(dir.path());
            chmod(dir.path(), 0o755);
            assert!(!result.unwrap(), "expected not accessible for mode 0o600");
            Ok(())
        }

        #[test]
        fn test_check_accessible_returns_execute_denied_for_no_execute_bit_dir() -> io::Result<()> {
            let dir = tempfile::tempdir()?;
            chmod(dir.path(), 0o600);
            let result = PermissionChecker::check_accessible(dir.path());
            chmod(dir.path(), 0o755);
            assert!(
                matches!(result, Err(PermissionError::ExecuteDenied(_))),
                "expected ExecuteDenied, got {:?}",
                result
            );
            Ok(())
        }

        #[test]
        fn test_is_readable_returns_true_for_owner_readable_file() -> io::Result<()> {
            // With mode 0o644 (owner read/write, group/other read), the owner
            // process should be able to open the file; confirm the attempt-based
            // check reports true.
            let file = NamedTempFile::new()?;
            let path = file.path().to_path_buf();
            chmod(&path, 0o644);
            let result = PermissionChecker::is_readable(&path).unwrap();
            assert!(result, "expected readable for mode 0o644");
            Ok(())
        }
    }
}
