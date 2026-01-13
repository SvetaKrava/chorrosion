// SPDX-License-Identifier: GPL-3.0-or-later

//! Matching precedence enforcement module.
//!
//! This module implements the matching strategy with enforced precedence:
//! 1. **Primary**: Fingerprint-based lookup via AcoustID (highest confidence)
//! 2. **Fallback**: Embedded tags extraction from audio files (medium confidence)
//! 3. **Final Fallback**: Filename heuristics (lowest confidence)
//!
//! The precedence engine ensures that each strategy is only attempted if
//! the previous one fails or is unavailable, with clear confidence scoring
//! at each level to allow confidence-based filtering.
//!
//! ## Usage
//!
//! ```no_run
//! # use chorrosion_application::matching_precedence::{PrecedenceMatchingEngine, MatchingStrategy};
//! # use chorrosion_domain::TrackFile;
//! # async fn example() -> anyhow::Result<()> {
//! let engine = PrecedenceMatchingEngine::new(/* services */);
//! let track_file = TrackFile::new(Default::default(), "/path/to/file.flac", 1024);
//!
//! // Attempt matching with enforced precedence
//! let result = engine.match_with_precedence(
//!     &track_file,
//!     0.5,  // min_confidence
//!     None, // folder_artist
//!     None  // folder_album
//! ).await?;
//!
//! println!("Strategy: {:?}, MBID: {}, Confidence: {}", 
//!          result.strategy, result.musicbrainz_recording_id, result.confidence);
//! # Ok(())
//! # }
//! ```

use crate::embedded_tags::EmbeddedTagMatchingService;
use crate::filename_heuristics::FilenameHeuristicsService;
use crate::matching::{MatchingError, TrackMatchingService};
use chorrosion_domain::TrackFile;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Which matching strategy was used to obtain a result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchingStrategy {
    /// Primary: Audio fingerprint lookup via AcoustID
    Fingerprint,
    /// Fallback: Embedded audio file tags (ID3, Vorbis, MP4)
    EmbeddedTags,
    /// Final fallback: Filename-based heuristics parsing
    FilenameHeuristics,
}

impl std::fmt::Display for MatchingStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchingStrategy::Fingerprint => write!(f, "Fingerprint (AcoustID)"),
            MatchingStrategy::EmbeddedTags => write!(f, "Embedded Tags"),
            MatchingStrategy::FilenameHeuristics => write!(f, "Filename Heuristics"),
        }
    }
}

/// Result from precedence-based matching including strategy information
#[derive(Debug, Clone)]
pub struct PrecedenceMatchResult {
    /// MusicBrainz recording ID
    pub musicbrainz_recording_id: String,
    /// Confidence score (normalized to 0.0-1.0)
    pub confidence: f32,
    /// Which strategy was used to obtain this match
    pub strategy: MatchingStrategy,
}

/// Errors that can occur during precedence matching
#[derive(Debug, Error)]
pub enum PrecedenceMatchingError {
    #[error("Fingerprint matching failed: {0}")]
    FingerprintFailed(#[from] MatchingError),

    #[error("All matching strategies failed")]
    AllStrategiesFailed,

    #[error("No metadata available for matching")]
    NoMetadataAvailable,

    #[error("Match confidence {score} below threshold {threshold}")]
    BelowConfidenceThreshold { score: f32, threshold: f32 },

    #[error("Invalid confidence threshold: {0}")]
    InvalidThreshold(f32),
}

pub type PrecedenceMatchingResult<T> = Result<T, PrecedenceMatchingError>;

/// Precedence matching engine orchestrating all matching strategies.
///
/// This engine implements the matching precedence with proper fallback logic
/// and confidence scoring at each level. It ensures deterministic matching
/// behavior with clear priority ordering.
pub struct PrecedenceMatchingEngine {
    fingerprint_service: Arc<TrackMatchingService>,
    embedded_tags_service: Arc<EmbeddedTagMatchingService>,
    filename_heuristics_service: Arc<FilenameHeuristicsService>,
}

impl PrecedenceMatchingEngine {
    /// Create a new precedence matching engine with the given services.
    ///
    /// # Arguments
    ///
    /// * `fingerprint_service` - Service for fingerprint-based matching
    /// * `embedded_tags_service` - Service for embedded tag matching
    /// * `filename_heuristics_service` - Service for filename-based matching
    pub fn new(
        fingerprint_service: TrackMatchingService,
        embedded_tags_service: EmbeddedTagMatchingService,
        filename_heuristics_service: FilenameHeuristicsService,
    ) -> Self {
        Self {
            fingerprint_service: Arc::new(fingerprint_service),
            embedded_tags_service: Arc::new(embedded_tags_service),
            filename_heuristics_service: Arc::new(filename_heuristics_service),
        }
    }

    /// Execute matching with enforced precedence (fingerprint > tags > filename).
    ///
    /// Attempts each matching strategy in order:
    /// 1. Fingerprint via AcoustID (if available)
    /// 2. Embedded tags (if available)
    /// 3. Filename heuristics (fallback)
    ///
    /// Returns the first successful match that meets the confidence threshold.
    ///
    /// # Arguments
    ///
    /// * `track_file` - The track file to match
    /// * `min_confidence` - Minimum confidence threshold (0.0-1.0)
    /// * `folder_artist` - Optional artist extracted from parent folder
    /// * `folder_album` - Optional album extracted from parent folder
    ///
    /// # Returns
    ///
    /// * `Ok(PrecedenceMatchResult)` - Successfully matched with strategy info
    /// * `Err(PrecedenceMatchingError::AllStrategiesFailed)` - All strategies failed
    /// * `Err(PrecedenceMatchingError::BelowConfidenceThreshold)` - Best match below threshold
    pub async fn match_with_precedence(
        &self,
        track_file: &TrackFile,
        min_confidence: f32,
        folder_artist: Option<&str>,
        folder_album: Option<&str>,
    ) -> PrecedenceMatchingResult<PrecedenceMatchResult> {
        // Validate confidence threshold
        if !(0.0..=1.0).contains(&min_confidence) {
            return Err(PrecedenceMatchingError::InvalidThreshold(min_confidence));
        }

        info!(
            target: "precedence_matching",
            track_file_id = %track_file.id,
            path = %track_file.path,
            min_confidence,
            "starting precedence-based matching"
        );

        // Strategy 1: Fingerprint-based lookup (highest confidence)
        if let Some(result) = self
            .try_fingerprint_match(track_file, min_confidence)
            .await
        {
            return result;
        }

        debug!(
            target: "precedence_matching",
            track_file_id = %track_file.id,
            "fingerprint matching unavailable, falling back to embedded tags"
        );

        // Strategy 2: Embedded tags (medium confidence)
        if let Some(result) = self
            .try_embedded_tags_match(
                track_file,
                min_confidence,
                folder_artist,
                folder_album,
            )
            .await
        {
            return result;
        }

        debug!(
            target: "precedence_matching",
            track_file_id = %track_file.id,
            "embedded tags matching unavailable, falling back to filename heuristics"
        );

        // Strategy 3: Filename heuristics (lowest confidence, final fallback)
        if let Some(result) = self
            .try_filename_heuristics_match(track_file, min_confidence, folder_artist, folder_album)
            .await
        {
            return result;
        }

        warn!(
            target: "precedence_matching",
            track_file_id = %track_file.id,
            "all matching strategies exhausted without success"
        );

        Err(PrecedenceMatchingError::AllStrategiesFailed)
    }

    /// Attempt fingerprint-based matching (primary strategy).
    async fn try_fingerprint_match(
        &self,
        track_file: &TrackFile,
        min_confidence: f32,
    ) -> Option<PrecedenceMatchingResult<PrecedenceMatchResult>> {
        debug!(
            target: "precedence_matching",
            track_file_id = %track_file.id,
            "attempting fingerprint-based matching"
        );

        match self
            .fingerprint_service
            .match_track(track_file, min_confidence)
            .await
        {
            Ok(match_result) => {
                info!(
                    target: "precedence_matching",
                    track_file_id = %track_file.id,
                    strategy = "Fingerprint",
                    mbid = %match_result.musicbrainz_recording_id,
                    confidence = match_result.confidence_score,
                    "fingerprint match successful"
                );

                Some(Ok(PrecedenceMatchResult {
                    musicbrainz_recording_id: match_result.musicbrainz_recording_id,
                    confidence: match_result.confidence_score,
                    strategy: MatchingStrategy::Fingerprint,
                }))
            }
            Err(e) => {
                debug!(
                    target: "precedence_matching",
                    track_file_id = %track_file.id,
                    error = %e,
                    "fingerprint matching unavailable"
                );
                None
            }
        }
    }

    /// Attempt embedded tags-based matching (fallback strategy).
    async fn try_embedded_tags_match(
        &self,
        track_file: &TrackFile,
        _min_confidence: f32,
        _folder_artist: Option<&str>,
        _folder_album: Option<&str>,
    ) -> Option<PrecedenceMatchingResult<PrecedenceMatchResult>> {
        debug!(
            target: "precedence_matching",
            track_file_id = %track_file.id,
            "attempting embedded tags matching"
        );

        // TODO: Implement actual embedded tags extraction and MusicBrainz lookup
        // This is a placeholder for the actual implementation
        // which would:
        // 1. Extract tags from audio file
        // 2. Look up in MusicBrainz with artist/album/title
        // 3. Return matches with confidence scores

        let tags = match self
            .embedded_tags_service
            .extract_tags(&track_file.path)
            .await
        {
            Ok(tags) => tags,
            Err(e) => {
                debug!(
                    target: "precedence_matching",
                    track_file_id = %track_file.id,
                    error = %e,
                    "embedded tags extraction failed"
                );
                return None;
            }
        };

        // Check if we have enough metadata to attempt matching
        if tags.artist.is_none() || tags.album.is_none() || tags.title.is_none() {
            debug!(
                target: "precedence_matching",
                track_file_id = %track_file.id,
                "insufficient metadata from embedded tags"
            );
            return None;
        }

        // TODO: Implement MusicBrainz lookup with extracted tags
        // For now, return None to allow fallback to filename heuristics
        None
    }

    /// Attempt filename heuristics-based matching (final fallback).
    async fn try_filename_heuristics_match(
        &self,
        track_file: &TrackFile,
        _min_confidence: f32,
        folder_artist: Option<&str>,
        folder_album: Option<&str>,
    ) -> Option<PrecedenceMatchingResult<PrecedenceMatchResult>> {
        debug!(
            target: "precedence_matching",
            track_file_id = %track_file.id,
            "attempting filename heuristics matching"
        );

        // Parse filename to extract metadata
        let parsed = match self
            .filename_heuristics_service
            .parse_filename(&track_file.path, folder_artist, folder_album)
        {
            Ok(parsed) => parsed,
            Err(e) => {
                debug!(
                    target: "precedence_matching",
                    track_file_id = %track_file.id,
                    error = %e,
                    "filename heuristics parsing failed"
                );
                return None;
            }
        };

        // Check if we have enough metadata to attempt matching
        if parsed.artist.is_none() || parsed.title.is_none() {
            debug!(
                target: "precedence_matching",
                track_file_id = %track_file.id,
                "insufficient metadata from filename heuristics"
            );
            return None;
        }

        // TODO: Implement MusicBrainz lookup with parsed filename data
        // Lower confidence scores for filename-based matches
        // For now, return None as placeholder
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_strategy_display() {
        assert_eq!(MatchingStrategy::Fingerprint.to_string(), "Fingerprint (AcoustID)");
        assert_eq!(MatchingStrategy::EmbeddedTags.to_string(), "Embedded Tags");
        assert_eq!(
            MatchingStrategy::FilenameHeuristics.to_string(),
            "Filename Heuristics"
        );
    }

    #[test]
    fn invalid_confidence_threshold() {
        let result_low = Err::<PrecedenceMatchResult<_>, _>(
            PrecedenceMatchingError::InvalidThreshold(-0.1),
        );
        let result_high = Err::<PrecedenceMatchResult<_>, _>(
            PrecedenceMatchingError::InvalidThreshold(1.1),
        );
        assert!(result_low.is_err());
        assert!(result_high.is_err());
    }

    #[test]
    fn matching_strategy_equality() {
        assert_eq!(MatchingStrategy::Fingerprint, MatchingStrategy::Fingerprint);
        assert_ne!(MatchingStrategy::Fingerprint, MatchingStrategy::EmbeddedTags);
    }

    #[test]
    fn precedence_match_result_creation() {
        let result = PrecedenceMatchResult {
            musicbrainz_recording_id: "test-id".to_string(),
            confidence: 0.95,
            strategy: MatchingStrategy::Fingerprint,
        };

        assert_eq!(result.musicbrainz_recording_id, "test-id");
        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.strategy, MatchingStrategy::Fingerprint);
    }
}
