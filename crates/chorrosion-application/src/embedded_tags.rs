// SPDX-License-Identifier: GPL-3.0-or-later

//! Embedded tag matching service (fallback strategy).
//!
//! This module provides basic matching using embedded audio metadata
//! (artist/album/track name from ID3/FLAC/Vorbis tags).
//!
//! Supports extraction from ID3v2 (MP3), Vorbis Comments (FLAC/OGG),
//! MP4 atoms (M4A), and APEv2 tags via the `lofty` audio library.

use crate::matching::MatchResult;
use lofty::file::TaggedFileExt;
use lofty::prelude::Accessor;
use std::path::Path;
use thiserror::Error;
use tracing::debug;

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

        // Read the metadata from the file using lofty
        let metadata = match lofty::read_from_path(path) {
            Ok(mtag) => mtag,
            Err(e) => {
                debug!(
                    target: "matching",
                    path = %path.display(),
                    format = %ext,
                    error = %e,
                    "failed to read metadata from audio file"
                );
                return Err(EmbeddedTagError::ExtractionFailed(format!(
                    "Failed to read metadata: {}",
                    e
                )));
            }
        };

        // Extract primary tag (most common tag type for the format)
        let tag = metadata
            .primary_tag()
            .or_else(|| metadata.first_tag())
            .ok_or(EmbeddedTagError::InsufficientMetadata)?;

        debug!(
            target: "matching",
            path = %path.display(),
            artist = ?tag.artist(),
            album = ?tag.album(),
            title = ?tag.title(),
            track_number = ?tag.track(),
            "extracted tags from audio file"
        );

        Ok(ExtractedTags {
            artist: tag.artist().map(|s| s.to_string()),
            album: tag.album().map(|s| s.to_string()),
            title: tag.title().map(|s| s.to_string()),
            track_number: tag.track(),
        })
    }

    /// Attempt to match using embedded tags from the given file path.
    ///
    /// Returns `Ok(None)` when no match can be determined due to missing tags
    /// or when tag parsing fails.
    ///
    /// # Note
    /// This extracts the tags successfully but returns None because the
    /// MusicBrainz matching logic for tagged metadata is still being
    /// implemented as part of the fingerprint-based lookup phase.
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
                // Search priority: (artist + album + track_number) > (artist + album) > artist
                // Return match with lower confidence than fingerprint matching
                // (fingerprint is more reliable when available)
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
    async fn returns_none_on_no_musicbrainz_match() {
        let svc = EmbeddedTagMatchingService;
        let test_file = std::env::current_dir().unwrap().join("Cargo.toml");
        let result = svc.match_from_file(&test_file).await;
        // Should return None since MusicBrainz matching is not yet implemented (deferred)
        assert!(matches!(result, Ok(None)));
    }

    #[tokio::test]
    async fn extract_tags_returns_error_for_non_audio_file() {
        let svc = EmbeddedTagMatchingService;
        let test_file = std::env::current_dir().unwrap().join("Cargo.toml");

        // Verify extract_tags returns an error for non-audio files
        let result = svc.extract_tags(&test_file).await;
        assert!(matches!(
            result,
            Err(EmbeddedTagError::ExtractionFailed(_) | EmbeddedTagError::InsufficientMetadata)
        ));
    }
}
