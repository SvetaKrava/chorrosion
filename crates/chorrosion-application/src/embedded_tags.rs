// SPDX-License-Identifier: GPL-3.0-or-later

//! Embedded tag matching service (fallback strategy).
//!
//! This module scaffolds a matching service that will parse
//! embedded audio metadata (ID3/FLAC/Vorbis) and attempt
//! to match tracks when fingerprints are unavailable.
//!
//! Implementation is intentionally minimal for now and returns
//! a placeholder result until tag parsing is wired.

use crate::matching::{MatchResult};
use thiserror::Error;
use tracing::{debug, warn};
use std::path::Path;

/// Errors that can occur during embedded tag matching
#[derive(Debug, Error)]
pub enum EmbeddedTagError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Tag parsing not yet implemented")]
    NotImplemented,
}

/// Result type used throughout the embedded tag matching module.
pub type EmbeddedTagResult<T> = Result<T, EmbeddedTagError>;

/// Fallback matching using embedded tags in audio files.
#[derive(Default, Clone)]
pub struct EmbeddedTagMatchingService;

impl EmbeddedTagMatchingService {
    /// Attempt to match using embedded tags from the given file path.
    ///
    /// Returns `Ok(None)` when no match can be determined.
    pub async fn match_from_file(&self, path: impl AsRef<Path>) -> EmbeddedTagResult<Option<MatchResult>> {
        let path = path.as_ref();
        debug!(target = "matching", path = %path.display(), "embedded tag matching invoked");

        if !path.exists() {
            return Err(EmbeddedTagError::FileNotFound(path.display().to_string()));
        }

        // Placeholder until tag parsing is implemented
        warn!(target = "matching", path = %path.display(), "embedded tag parsing not implemented yet");
        Err(EmbeddedTagError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_file_not_found_error() {
        let svc = EmbeddedTagMatchingService;
        let result = svc.match_from_file("does_not_exist.mp3").await;
        assert!(matches!(result, Err(EmbeddedTagError::FileNotFound(_))));
    }

    #[tokio::test]
    async fn returns_not_implemented_for_existing_file() {
        let svc = EmbeddedTagMatchingService;
        let test_file = std::env::current_dir().unwrap().join("Cargo.toml");
        let result = svc.match_from_file(test_file).await;
        assert!(matches!(result, Err(EmbeddedTagError::NotImplemented)));
    }
}
