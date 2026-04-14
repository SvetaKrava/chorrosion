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
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::debug;

/// Errors that can occur during embedded tag matching
#[derive(Debug, Error)]
pub enum EmbeddedTagError {
    #[error("File not found: {0}")]
    FileNotFound(String),

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
    /// Supports the following formats via the `lofty` audio library:
    /// - ID3v2 tags (MP3)
    /// - Vorbis comments (OGG, FLAC)
    /// - MP4 atoms (M4A)
    /// - APEv2 tags
    ///
    /// The blocking file I/O is offloaded to a thread pool via
    /// `tokio::task::spawn_blocking` so it does not stall the async runtime.
    ///
    /// # Arguments
    /// * `path` - Path to the audio file
    ///
    /// # Returns
    /// * `Ok(ExtractedTags)` - Successfully extracted tags
    /// * `Err(EmbeddedTagError::FileNotFound)` - File does not exist
    /// * `Err(EmbeddedTagError::ExtractionFailed)` - lofty could not parse the file
    /// * `Err(EmbeddedTagError::InsufficientMetadata)` - File parsed but has no tag block
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

        // Offload blocking file I/O + tag parsing to the thread pool so the
        // async runtime worker thread is not blocked during imports/matching.
        let owned_path: PathBuf = path.to_path_buf();
        let tags_result = tokio::task::spawn_blocking(move || {
            lofty::read_from_path(&owned_path).map_err(|e| {
                EmbeddedTagError::ExtractionFailed(format!("Failed to read metadata: {}", e))
            })
        })
        .await
        .map_err(|e| EmbeddedTagError::ExtractionFailed(format!("Task join error: {}", e)))?;

        let metadata = match tags_result {
            Ok(mtag) => mtag,
            Err(e) => {
                debug!(
                    target: "matching",
                    path = %path.display(),
                    format = %ext,
                    error = %e,
                    "failed to read metadata from audio file"
                );
                return Err(e);
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
            format = %ext,
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
    use std::fs;

    // ── Minimal valid audio fixtures ─────────────────────────────────────────
    //
    // Identical to those used in tag_embedding.rs; duplicated here so this
    // module stays self-contained and both sets of tests remain independent.

    /// Minimal valid MPEG/MP3 file (two MPEG1-L3 frames at 32 kbps/44100 Hz).
    const MINIMAL_MP3: &[u8] = &{
        const FRAME_HDR: [u8; 4] = [0xFF, 0xFB, 0x10, 0x44];
        let mut b = [0u8; 218]; // 10-byte ID3 header + 2 × 104-byte MPEG frames
        // ID3v2.4 header at offset 0 (10 bytes, size field = 0)
        b[0] = b'I';
        b[1] = b'D';
        b[2] = b'3';
        b[3] = 4; // version: ID3v2.4
        // Frame 1 at offset 10 (frame_length = floor(1152×32000/(8×44100)) = 104 bytes)
        b[10] = FRAME_HDR[0];
        b[11] = FRAME_HDR[1];
        b[12] = FRAME_HDR[2];
        b[13] = FRAME_HDR[3];
        // Frame 2 at offset 10 + 104 = 114
        b[114] = FRAME_HDR[0];
        b[115] = FRAME_HDR[1];
        b[116] = FRAME_HDR[2];
        b[117] = FRAME_HDR[3];
        b
    };

    /// Minimal valid FLAC stream (STREAMINFO + empty PADDING block).
    const MINIMAL_FLAC: &[u8] = &[
        b'f', b'L', b'a', b'C',
        0x00, 0x00, 0x00, 0x22,
        0x00, 0x10, 0x00, 0x10,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x0A, 0xC4, 0x40, 0xF0, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x81, 0x00, 0x00, 0x00,
    ];

    fn write_fixture(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, bytes).expect("fixture write");
        path
    }

    /// Write known artist/album/title/track tags to a file using lofty, then
    /// return the path so tests can call `extract_tags` on it.
    fn embed_known_tags(path: &PathBuf) {
        use lofty::config::WriteOptions;
        use lofty::file::{AudioFile, TaggedFileExt};
        use lofty::prelude::Accessor;
        use lofty::probe::Probe;

        let mut tagged = Probe::open(path)
            .expect("probe open")
            .guess_file_type()
            .expect("guess type")
            .read()
            .expect("read tagged file");

        let tag = if let Some(t) = tagged.primary_tag_mut() {
            t
        } else {
            let tag_type = tagged.primary_tag_type();
            tagged.insert_tag(lofty::tag::Tag::new(tag_type));
            tagged.primary_tag_mut().expect("tag inserted")
        };

        tag.set_artist("Test Artist".to_string());
        tag.set_album("Test Album".to_string());
        tag.set_title("Test Title".to_string());
        tag.set_track(3);

        tagged
            .save_to_path(path, WriteOptions::default())
            .expect("save tags");
    }

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

    #[tokio::test]
    async fn extract_tags_mp3_returns_known_tags() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_fixture(&dir, "track.mp3", MINIMAL_MP3);
        embed_known_tags(&path);

        let svc = EmbeddedTagMatchingService;
        let tags = svc.extract_tags(&path).await.expect("extract should succeed");

        assert_eq!(tags.artist.as_deref(), Some("Test Artist"));
        assert_eq!(tags.album.as_deref(), Some("Test Album"));
        assert_eq!(tags.title.as_deref(), Some("Test Title"));
        assert_eq!(tags.track_number, Some(3));
    }

    #[tokio::test]
    async fn extract_tags_flac_returns_known_tags() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_fixture(&dir, "track.flac", MINIMAL_FLAC);
        embed_known_tags(&path);

        let svc = EmbeddedTagMatchingService;
        let tags = svc.extract_tags(&path).await.expect("extract should succeed");

        assert_eq!(tags.artist.as_deref(), Some("Test Artist"));
        assert_eq!(tags.album.as_deref(), Some("Test Album"));
        assert_eq!(tags.title.as_deref(), Some("Test Title"));
        assert_eq!(tags.track_number, Some(3));
    }
}
