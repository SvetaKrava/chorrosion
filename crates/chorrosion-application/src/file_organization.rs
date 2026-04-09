// SPDX-License-Identifier: GPL-3.0-or-later

use crate::permission::{PermissionChecker, PermissionConfig, PermissionManager};
use lazy_static::lazy_static;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::trace;

lazy_static! {
    static ref TOKEN_REGEX: Regex = Regex::new(r"\{(?P<token>[a-z]+(?::\d+)?)\}")
        .expect("failed to compile token replacement regex pattern");
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOperationMode {
    Copy,
    Move,
    Hardlink,
}

#[derive(Debug, Clone)]
pub struct TrackPathContext {
    pub artist: String,
    pub album: String,
    pub title: String,
    pub extension: String,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
}

#[derive(Debug, Error)]
pub enum FileOrganizationError {
    #[error("source path does not exist: {0}")]
    SourceNotFound(String),
    #[error("target already exists: {0}")]
    TargetExists(String),
    #[error("invalid naming pattern: {0}")]
    InvalidPattern(String),
    #[error("file operation failed: {0}")]
    FileOperation(String),
    #[error("permission denied: {0}")]
    Permission(String),
}

pub fn render_naming_pattern(
    pattern: &str,
    context: &TrackPathContext,
) -> Result<String, FileOrganizationError> {
    if pattern.trim().is_empty() {
        return Err(FileOrganizationError::InvalidPattern(
            "pattern cannot be empty".to_string(),
        ));
    }

    let rendered = TOKEN_REGEX
        .replace_all(pattern, |captures: &regex::Captures| {
            resolve_token(
                captures.name("token").map(|m| m.as_str()).unwrap_or(""),
                context,
            )
        })
        .into_owned();

    Ok(rendered)
}

pub fn build_organized_file_path(
    base: &Path,
    folder_pattern: &str,
    file_pattern: &str,
    context: &TrackPathContext,
) -> Result<PathBuf, FileOrganizationError> {
    let rendered_folder = render_naming_pattern(folder_pattern, context)?;
    let rendered_file_stem = sanitize_component(&render_naming_pattern(file_pattern, context)?);

    if rendered_file_stem.is_empty() {
        return Err(FileOrganizationError::InvalidPattern(
            "rendered file name is empty".to_string(),
        ));
    }

    let extension = context.extension.trim_start_matches('.').to_string();
    let file_name = if extension.is_empty() {
        rendered_file_stem
    } else {
        format!("{}.{}", rendered_file_stem, extension)
    };

    let mut path = PathBuf::from(base);
    for segment in rendered_folder.split(['/', '\\']) {
        let segment = sanitize_component(segment);
        if !segment.is_empty() {
            path.push(segment);
        }
    }

    if context.disc_number.unwrap_or(1) > 1 {
        path.push(format!("Disc {:02}", context.disc_number.unwrap_or(1)));
    }

    path.push(file_name);
    Ok(path)
}

pub fn apply_file_operation(
    source: &Path,
    destination: &Path,
    mode: FileOperationMode,
    overwrite: bool,
    permission_config: Option<&PermissionConfig>,
) -> Result<(), FileOrganizationError> {
    if !source.exists() {
        return Err(FileOrganizationError::SourceNotFound(
            source.display().to_string(),
        ));
    }

    // Check that the source file is readable before attempting any file operation.
    if permission_config.is_some() {
        PermissionChecker::check_readable(source)
            .map_err(|e| FileOrganizationError::Permission(e.to_string()))?;
    }

    // Guard against source and destination being the same file to prevent data loss.
    let canonical_source = source
        .canonicalize()
        .map_err(|err| FileOrganizationError::FileOperation(err.to_string()))?;
    if let Ok(canonical_dest) = destination.canonicalize() {
        if canonical_source == canonical_dest {
            trace!(
                target: "application",
                "source and destination resolve to the same path, skipping file operation"
            );
            return Ok(());
        }
    }

    // For Move with permission preservation: save source permissions before the
    // operation because the source will no longer exist afterward.
    let saved_permissions = match (permission_config, &mode) {
        (Some(config), FileOperationMode::Move) if config.preserve_permissions => Some(
            PermissionManager::get_permissions(source)
                .map_err(|e| FileOrganizationError::Permission(e.to_string()))?,
        ),
        _ => None,
    };

    if destination.exists() {
        if !overwrite {
            return Err(FileOrganizationError::TargetExists(
                destination.display().to_string(),
            ));
        }
        fs::remove_file(destination)
            .map_err(|err| FileOrganizationError::FileOperation(err.to_string()))?;
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| FileOrganizationError::FileOperation(err.to_string()))?;
    }

    match mode {
        FileOperationMode::Copy => {
            fs::copy(source, destination)
                .map_err(|err| FileOrganizationError::FileOperation(err.to_string()))?;
        }
        FileOperationMode::Hardlink => {
            fs::hard_link(source, destination)
                .map_err(|err| FileOrganizationError::FileOperation(err.to_string()))?;
        }
        FileOperationMode::Move => {
            if let Err(rename_error) = fs::rename(source, destination) {
                fs::copy(source, destination).map_err(|copy_error| {
                    FileOrganizationError::FileOperation(copy_error.to_string())
                })?;
                fs::remove_file(source).map_err(|remove_error| {
                    FileOrganizationError::FileOperation(format!(
                        "failed to remove source after move fallback (rename error: {}, remove error: {})",
                        rename_error, remove_error
                    ))
                })?;
            }
        }
    }

    // Apply permissions to the destination after the file operation.
    if let Some(config) = permission_config {
        match mode {
            FileOperationMode::Move => {
                if let Some(perms) = saved_permissions {
                    // Restore permissions saved before the move (source is now gone).
                    fs::set_permissions(destination, perms)
                        .map_err(|e| FileOrganizationError::Permission(e.to_string()))?;
                } else {
                    // Source is gone; apply configured defaults.
                    PermissionManager::apply_defaults(destination, config)
                        .map_err(|e| FileOrganizationError::Permission(e.to_string()))?;
                }
            }
            FileOperationMode::Copy | FileOperationMode::Hardlink => {
                // Source still exists; preserve from it or apply defaults per config.
                PermissionManager::apply_permissions(source, destination, config)
                    .map_err(|e| FileOrganizationError::Permission(e.to_string()))?;
            }
        }
    }

    Ok(())
}

fn resolve_token(token: &str, context: &TrackPathContext) -> String {
    match token {
        "artist" => sanitize_component(&context.artist),
        "album" => sanitize_component(&context.album),
        "title" => sanitize_component(&context.title),
        "ext" => context.extension.trim_start_matches('.').to_string(),
        "track" => context
            .track_number
            .map(|number| number.to_string())
            .unwrap_or_default(),
        "track:02" => context
            .track_number
            .map(|number| format!("{:02}", number))
            .unwrap_or_default(),
        "disc" => context
            .disc_number
            .map(|number| number.to_string())
            .unwrap_or_default(),
        "disc:02" => context
            .disc_number
            .map(|number| format!("{:02}", number))
            .unwrap_or_default(),
        _ => token.to_string(),
    }
}

fn sanitize_component(input: &str) -> String {
    let banned = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

    // Replace banned characters with spaces and normalize whitespace.
    let sanitized = input
        .chars()
        .map(|character| {
            if banned.contains(&character) {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Trim leading/trailing spaces and dots (prevents Windows issues and neutralizes
    // components that reduce to "." or "..").
    let mut component = sanitized
        .trim_matches(|c: char| c == ' ' || c == '.')
        .to_string();

    // Explicitly neutralize path traversal components that survived trimming.
    if component == "." || component == ".." {
        return String::new();
    }

    // Reject Windows reserved device names to keep paths filesystem-safe.
    // Reserved names are invalid even with an extension (e.g., "con.txt"),
    // so we check the stem (portion before the first dot) and insert '_' there.
    if !component.is_empty() {
        let stem = component.split('.').next().unwrap_or(&component);
        let lower_stem = stem.to_ascii_lowercase();
        let is_reserved = matches!(
            lower_stem.as_str(),
            "con"
                | "prn"
                | "aux"
                | "nul"
                | "com1"
                | "com2"
                | "com3"
                | "com4"
                | "com5"
                | "com6"
                | "com7"
                | "com8"
                | "com9"
                | "lpt1"
                | "lpt2"
                | "lpt3"
                | "lpt4"
                | "lpt5"
                | "lpt6"
                | "lpt7"
                | "lpt8"
                | "lpt9"
        );
        if is_reserved {
            // Insert '_' after the stem to make "con" → "con_" and "con.txt" → "con_.txt".
            component.insert(stem.len(), '_');
        }
    }

    component
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_context() -> TrackPathContext {
        TrackPathContext {
            artist: "Boards of Canada".to_string(),
            album: "Music Has the Right to Children".to_string(),
            title: "Roygbiv".to_string(),
            extension: "flac".to_string(),
            track_number: Some(4),
            disc_number: Some(1),
        }
    }

    #[test]
    fn renders_tokens_with_padding() {
        let rendered = render_naming_pattern("{track:02} - {title}", &sample_context())
            .expect("render should succeed");
        assert_eq!(rendered, "04 - Roygbiv");
    }

    #[test]
    fn unknown_token_is_preserved_in_rendered_output() {
        let rendered = render_naming_pattern("{unknown} - {title}", &sample_context())
            .expect("render should succeed");
        assert_eq!(rendered, "unknown - Roygbiv");
    }

    #[test]
    fn builds_multi_disc_path_when_disc_number_is_greater_than_one() {
        let mut context = sample_context();
        context.disc_number = Some(2);
        let base = PathBuf::from("/music");

        let path =
            build_organized_file_path(&base, "{artist}/{album}", "{track:02} - {title}", &context)
                .expect("path build should succeed");

        let expected_suffix = PathBuf::from("Boards of Canada")
            .join("Music Has the Right to Children")
            .join("Disc 02")
            .join("04 - Roygbiv.flac");
        assert!(path.ends_with(expected_suffix));
    }

    #[test]
    fn copy_operation_creates_destination_and_keeps_source() {
        let temp_dir = tempdir().expect("temp directory should be created");
        let source = temp_dir.path().join("source.flac");
        let destination = temp_dir.path().join("library").join("dest.flac");
        fs::write(&source, b"audio-data").expect("source should be written");

        apply_file_operation(&source, &destination, FileOperationMode::Copy, false, None)
            .expect("copy should succeed");

        assert!(source.exists());
        assert!(destination.exists());
    }

    #[test]
    fn move_operation_transfers_file() {
        let temp_dir = tempdir().expect("temp directory should be created");
        let source = temp_dir.path().join("source.mp3");
        let destination = temp_dir.path().join("organized").join("dest.mp3");
        fs::write(&source, b"audio-data").expect("source should be written");

        apply_file_operation(&source, &destination, FileOperationMode::Move, false, None)
            .expect("move should succeed");

        assert!(!source.exists());
        assert!(destination.exists());
    }

    #[test]
    fn hardlink_operation_creates_link() {
        let temp_dir = tempdir().expect("temp directory should be created");
        let source = temp_dir.path().join("source.flac");
        let destination = temp_dir.path().join("organized").join("linked.flac");
        fs::write(&source, b"audio-data").expect("source should be written");

        apply_file_operation(
            &source,
            &destination,
            FileOperationMode::Hardlink,
            false,
            None,
        )
        .expect("hardlink should succeed");

        assert!(source.exists());
        assert!(destination.exists());
    }

    #[test]
    fn target_exists_without_overwrite_returns_error() {
        let temp_dir = tempdir().expect("temp directory should be created");
        let source = temp_dir.path().join("source.flac");
        let destination = temp_dir.path().join("dest.flac");
        fs::write(&source, b"audio-data").expect("source should be written");
        fs::write(&destination, b"existing").expect("dest should be written");

        let result =
            apply_file_operation(&source, &destination, FileOperationMode::Copy, false, None);
        assert!(matches!(
            result,
            Err(FileOrganizationError::TargetExists(_))
        ));
    }

    #[test]
    fn permission_check_fails_on_missing_source() {
        let temp_dir = tempdir().expect("temp directory should be created");
        let source = temp_dir.path().join("nonexistent.flac");
        let destination = temp_dir.path().join("dest.flac");
        let config = crate::permission::PermissionConfig::default();

        let result = apply_file_operation(
            &source,
            &destination,
            FileOperationMode::Copy,
            false,
            Some(&config),
        );
        assert!(matches!(
            result,
            Err(FileOrganizationError::SourceNotFound(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn copy_with_permission_config_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempdir().expect("temp directory should be created");
        let source = temp_dir.path().join("source.flac");
        let destination = temp_dir.path().join("dest.flac");
        fs::write(&source, b"audio-data").expect("source should be written");

        // Set a specific permission on the source.
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644))
            .expect("should set permissions");

        let config = crate::permission::PermissionConfig {
            preserve_permissions: true,
            ..Default::default()
        };
        apply_file_operation(
            &source,
            &destination,
            FileOperationMode::Copy,
            false,
            Some(&config),
        )
        .expect("copy should succeed");

        let dest_mode = fs::metadata(&destination)
            .expect("dest metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dest_mode, 0o644);
    }

    #[cfg(unix)]
    #[test]
    fn copy_with_permission_config_applies_defaults_when_preserve_disabled() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempdir().expect("temp directory should be created");
        let source = temp_dir.path().join("source.flac");
        let destination = temp_dir.path().join("dest.flac");
        fs::write(&source, b"audio-data").expect("source should be written");

        let config = crate::permission::PermissionConfig {
            preserve_permissions: false,
            file_mode: 0o600,
            dir_mode: 0o700,
        };
        apply_file_operation(
            &source,
            &destination,
            FileOperationMode::Copy,
            false,
            Some(&config),
        )
        .expect("copy should succeed");

        let dest_mode = fs::metadata(&destination)
            .expect("dest metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dest_mode, 0o600);
    }
}
