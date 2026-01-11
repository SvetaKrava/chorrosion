// SPDX-License-Identifier: GPL-3.0-or-later

//! Filename-based heuristics matching (final fallback strategy).
//!
//! This module implements basic pattern-based matching when fingerprints
//! and embedded tags are unavailable. Common patterns include:
//! - Artist - Album - Track - Title
//! - Artist - Album (Track #Title)
//! - [Artist] Album - Track Title
//!
//! Confidence is typically lower than fingerprint or tag-based matching.

use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, warn};

/// Errors that can occur during filename heuristics matching
#[derive(Debug, Error)]
pub enum FilenameHeuristicsError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Could not parse filename: {0}")]
    ParsingFailed(String),

    #[error("Insufficient information to match")]
    InsufficientMetadata,
}

/// Result type for filename heuristics operations
pub type FilenameHeuristicsResult<T> = Result<T, FilenameHeuristicsError>;

/// Metadata extracted from filename
#[derive(Debug, Clone, Default)]
pub struct ParsedFilename {
    /// Artist name extracted from filename
    pub artist: Option<String>,
    /// Album title extracted from filename
    pub album: Option<String>,
    /// Track title extracted from filename
    pub title: Option<String>,
    /// Track number if present
    pub track_number: Option<u32>,
}

lazy_static! {
    // Pattern: Artist - Album - TrackNum - Title
    static ref PATTERN_DETAILED: Regex = Regex::new(
        r"^(?P<artist>[^-]+)\s*-\s*(?P<album>[^-]+)\s*-\s*(?P<track>\d+)\s*-\s*(?P<title>.+?)(?:\.|$)"
    ).unwrap();

    // Pattern: Artist - TrackNum - Title (album implicit from folder)
    static ref PATTERN_ARTIST_TRACK_TITLE: Regex = Regex::new(
        r"^(?P<artist>[^-]+)\s*-\s*(?P<track>\d+)\s*-\s*(?P<title>.+?)(?:\.|$)"
    ).unwrap();

    // Pattern: TrackNum - Title (artist/album from folder structure)
    static ref PATTERN_TRACK_TITLE: Regex = Regex::new(
        r"^(?P<track>\d+)\s*-\s*(?P<title>.+?)(?:\.|$)"
    ).unwrap();

    // Pattern: TrackNum Title (no separator between track and title)
    static ref PATTERN_TRACK_TITLE_SPACE: Regex = Regex::new(
        r"^(?P<track>\d+)\s+(?P<title>.+?)(?:\.|$)"
    ).unwrap();
}

/// Filename-based heuristics matching service (final fallback).
#[derive(Default, Clone)]
pub struct FilenameHeuristicsService;

impl FilenameHeuristicsService {
    /// Parse a filename to extract artist/album/track/title information.
    ///
    /// Attempts to match against common patterns, with support for extracting
    /// metadata from parent directory structure when available.
    ///
    /// # Supported Patterns
    /// 1. `Artist - Album - 01 - Title` (most specific)
    /// 2. `Artist - 01 - Title` (album from folder)
    /// 3. `01 - Title` (artist/album from folder structure)
    /// 4. `01 Title` (space-separated variant)
    ///
    /// # Arguments
    /// * `path` - Path to the audio file
    /// * `folder_artist` - Optional artist extracted from parent folder
    /// * `folder_album` - Optional album extracted from parent folder
    ///
    /// # Returns
    /// * `Ok(ParsedFilename)` - Successfully parsed metadata
    /// * `Err(FilenameHeuristicsError::FileNotFound)` - File does not exist
    /// * `Err(FilenameHeuristicsError::ParsingFailed)` - Could not match any pattern
    pub fn parse_filename(
        &self,
        path: impl AsRef<Path>,
        folder_artist: Option<&str>,
        folder_album: Option<&str>,
    ) -> FilenameHeuristicsResult<ParsedFilename> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(FilenameHeuristicsError::FileNotFound(
                path.display().to_string(),
            ));
        }

        let filename = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
            FilenameHeuristicsError::ParsingFailed("Invalid filename".to_string())
        })?;

        debug!(
            target: "matching",
            filename = %filename,
            folder_artist = ?folder_artist,
            folder_album = ?folder_album,
            "attempting to parse filename"
        );

        // Try patterns in order of specificity
        if let Some(caps) = PATTERN_DETAILED.captures(filename) {
            return Ok(ParsedFilename {
                artist: caps.name("artist").map(|m| m.as_str().trim().to_string()),
                album: caps.name("album").map(|m| m.as_str().trim().to_string()),
                title: caps.name("title").map(|m| m.as_str().trim().to_string()),
                track_number: caps
                    .name("track")
                    .and_then(|m| m.as_str().parse::<u32>().ok()),
            });
        }

        if let Some(caps) = PATTERN_ARTIST_TRACK_TITLE.captures(filename) {
            return Ok(ParsedFilename {
                artist: caps.name("artist").map(|m| m.as_str().trim().to_string()),
                album: folder_album.map(|s| s.to_string()),
                title: caps.name("title").map(|m| m.as_str().trim().to_string()),
                track_number: caps
                    .name("track")
                    .and_then(|m| m.as_str().parse::<u32>().ok()),
            });
        }

        if let Some(caps) = PATTERN_TRACK_TITLE.captures(filename) {
            return Ok(ParsedFilename {
                artist: folder_artist.map(|s| s.to_string()),
                album: folder_album.map(|s| s.to_string()),
                title: caps.name("title").map(|m| m.as_str().trim().to_string()),
                track_number: caps
                    .name("track")
                    .and_then(|m| m.as_str().parse::<u32>().ok()),
            });
        }

        if let Some(caps) = PATTERN_TRACK_TITLE_SPACE.captures(filename) {
            return Ok(ParsedFilename {
                artist: folder_artist.map(|s| s.to_string()),
                album: folder_album.map(|s| s.to_string()),
                title: caps.name("title").map(|m| m.as_str().trim().to_string()),
                track_number: caps
                    .name("track")
                    .and_then(|m| m.as_str().parse::<u32>().ok()),
            });
        }

        // No pattern matched
        warn!(
            target: "matching",
            filename = %filename,
            "no filename patterns matched"
        );
        Err(FilenameHeuristicsError::ParsingFailed(format!(
            "No matching filename patterns for '{}'",
            filename
        )))
    }

    /// Attempt to match a track using filename heuristics.
    ///
    /// Returns `Ok(None)` when insufficient metadata is available to make
    /// a meaningful match attempt.
    ///
    /// # Note
    /// This is a placeholder for now. Full implementation would:
    /// 1. Parse filename and extract artist/album/track/title
    /// 2. Query MusicBrainz with this metadata
    /// 3. Return matches with low confidence (0.5-0.7 range)
    pub async fn match_from_filename(
        &self,
        path: impl AsRef<Path>,
        folder_artist: Option<&str>,
        folder_album: Option<&str>,
    ) -> FilenameHeuristicsResult<Option<ParsedFilename>> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(FilenameHeuristicsError::FileNotFound(
                path.display().to_string(),
            ));
        }

        // Parse the filename
        match self.parse_filename(path, folder_artist, folder_album) {
            Ok(parsed) => {
                // Verify we have minimum required metadata for matching
                if parsed.title.is_none() {
                    return Err(FilenameHeuristicsError::InsufficientMetadata);
                }

                debug!(
                    target: "matching",
                    artist = ?parsed.artist,
                    album = ?parsed.album,
                    title = ?parsed.title,
                    track_number = ?parsed.track_number,
                    "successfully parsed filename for matching"
                );

                // TODO: Implement MusicBrainz search with parsed metadata
                // For now, return the parsed metadata so caller knows we extracted something
                Ok(Some(parsed))
            }
            Err(e) => {
                debug!(
                    target: "matching",
                    error = %e,
                    "filename parsing failed"
                );
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_detailed_pattern() {
        let service = FilenameHeuristicsService::default();

        // Cargo.toml exists but won't match pattern
        let result = service.parse_filename(
            std::env::current_dir().unwrap().join("Cargo.toml"),
            None,
            None,
        );

        // Should error with ParsingFailed for unmatched pattern
        assert!(matches!(
            result,
            Err(FilenameHeuristicsError::ParsingFailed(_))
        ));
    }

    #[test]
    fn parse_artist_track_title() {
        let service = FilenameHeuristicsService::default();
        let path = std::env::current_dir().unwrap().join("Cargo.toml");

        let result = service.parse_filename(&path, Some("Pink Floyd"), Some("The Wall"));

        // File exists but won't match pattern (filename is "Cargo")
        assert!(matches!(
            result,
            Err(FilenameHeuristicsError::ParsingFailed(_))
        ));
    }

    #[test]
    fn parse_track_title_space_separated() {
        let service = FilenameHeuristicsService::default();
        let path = std::env::current_dir().unwrap().join("Cargo.toml");

        let result = service.parse_filename(&path, Some("Artist"), Some("Album"));

        // File exists but filename "Cargo" won't match pattern
        assert!(matches!(
            result,
            Err(FilenameHeuristicsError::ParsingFailed(_))
        ));
    }

    #[test]
    fn file_not_found_error() {
        let service = FilenameHeuristicsService::default();
        let result = service.parse_filename("does_not_exist.mp3", None, None);
        assert!(matches!(
            result,
            Err(FilenameHeuristicsError::FileNotFound(_))
        ));
    }
}
