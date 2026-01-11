// SPDX-License-Identifier: GPL-3.0-or-later

//! Embedded tag matching service (fallback strategy).
//!
//! This module provides basic matching using embedded audio metadata
//! (artist/album/track name from ID3/FLAC/Vorbis tags).
//!
//! Note: This is a placeholder implementation. Full tag extraction requires
//! external audio libraries (metaflac, id3, etc.). For now, we provide the
//! infrastructure and error handling.

use crate::matching::MatchResult;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, warn};

/// Errors that can occur during embedded tag matching
#[derive(Debug, Error)]
pub enum EmbeddedTagError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Tag parsing not implemented: {0}")]
    NotImplemented(String),

    #[error("Failed to extract tags: {0}")]
    ExtractionFailed(String),

    #[error("Insufficient metadata to match")]
    InsufficientMetadata,
}

/// Result type used throughout the embedded tag matching module.
pub type EmbeddedTagResult<T> = Result<T, EmbeddedTagError>;

/// Extracted metadata from audio file tags
#[derive(Debug, Clone, Default)]
pub struct ExtractedTags {
    /// Artist name from tags
    pub artist: Option<String>,
    /// Album title from tags
    pub album: Option<String>,
    /// Track title from tags
    pub title: Option<String>,
    /// Track number from tags
    pub track_number: Option<u32>,
}

/// Fallback matching using embedded tags in audio files.
#[derive(Default, Clone)]
pub struct EmbeddedTagMatchingService;

impl EmbeddedTagMatchingService {
    /// Extract metadata from embedded tags in an audio file.
    ///
    /// Currently returns `NotImplemented` as this requires external audio libraries.
    /// Future implementation will support:
    /// - ID3v2 tags (MP3)
    /// - Vorbis comments (OGG, FLAC)
    /// - MP4 atoms (M4A)
    /// - APEv2 tags
    ///
    /// # Arguments
    /// * `path` - Path to the audio file
    ///
    /// # Returns
    /// * `Ok(ExtractedTags)` - Successfully extracted tags
    /// * `Err(EmbeddedTagError::FileNotFound)` - File does not exist
    /// * `Err(EmbeddedTagError::NotImplemented)` - Tag parsing not yet available
    pub async fn extract_tags(&self, path: impl AsRef<Path>) -> EmbeddedTagResult<ExtractedTags> {
        let path = path.as_ref();
        debug!(target: "matching", path = %path.display(), "attempting to extract embedded tags");

        if !path.exists() {
            return Err(EmbeddedTagError::FileNotFound(path.display().to_string()));
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Placeholder: Tag extraction not yet implemented
        warn!(
            target: "matching",
            path = %path.display(),
            format = %ext,
            "embedded tag extraction not yet implemented"
        );

        Err(EmbeddedTagError::NotImplemented(
            "Tag parsing requires external audio libraries (metaflac, id3, etc.)".to_string(),
        ))
    }

    /// Attempt to match using embedded tags from the given file path.
    ///
    /// Returns `Ok(None)` when no match can be determined due to missing tags
    /// or when tag parsing is not yet implemented.
    ///
    /// # Note
    /// This is a placeholder that returns None until tag parsing is implemented.
    /// The actual matching logic would:
    /// 1. Extract artist, album, and track name
    /// 2. Search MusicBrainz API with this metadata
    /// 3. Return the best match (lower confidence than fingerprint matching)
    pub async fn match_from_file(
        &self,
        path: impl AsRef<Path>,
    ) -> EmbeddedTagResult<Option<MatchResult>> {
        let path = path.as_ref();
        debug!(target: "matching", path = %path.display(), "embedded tag matching invoked");

        if !path.exists() {
            return Err(EmbeddedTagError::FileNotFound(path.display().to_string()));
        }

        // Try to extract tags
        match self.extract_tags(path).await {
            Ok(tags) => {
                debug!(
                    target: "matching",
                    artist = ?tags.artist,
                    album = ?tags.album,
                    title = ?tags.title,
                    "successfully extracted tags"
                );

                // TODO: Implement MusicBrainz search with extracted metadata
                // For now, return None (match deferred to filename heuristics)
                Ok(None)
            }
            Err(EmbeddedTagError::FileNotFound(_)) => {
                Err(EmbeddedTagError::FileNotFound(path.display().to_string()))
            }
            Err(e) => {
                debug!(target: "matching", error = %e, "tag extraction failed, deferring to filename heuristics");
                // Tag extraction failed; return None to let filename heuristics try
                Ok(None)
            }
        }
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
    async fn returns_none_on_extraction_not_implemented() {
        let svc = EmbeddedTagMatchingService;
        let test_file = std::env::current_dir().unwrap().join("Cargo.toml");
        let result = svc.match_from_file(&test_file).await;
        // Should return None since extraction is not yet implemented
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn extract_tags_not_yet_implemented() {
        let test_file = std::env::current_dir().unwrap().join("Cargo.toml");
        // Extract tags test will be implemented when tag parsing libraries are added
        assert!(test_file.exists());
    }
}
