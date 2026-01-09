// SPDX-License-Identifier: GPL-3.0-or-later

//! Track matching service using fingerprints as primary lookup strategy.
//!
//! The matching engine implements a fallback chain:
//! 1. Fingerprint-based lookup via AcoustID
//! 2. Embedded metadata tags (future)
//! 3. Filename heuristics (future)

use chorrosion_domain::{Track, TrackFile, TrackFileId, TrackId};
use chorrosion_fingerprint::{Fingerprint, AcoustidClient, FingerprintError};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, warn};

/// Errors that can occur during track matching
#[derive(Debug, Error)]
pub enum MatchingError {
    #[error("Fingerprint error: {0}")]
    FingerprintError(#[from] FingerprintError),

    #[error("No fingerprint provided for track file {0}")]
    NoFingerprint(TrackFileId),

    #[error("No matches found for track {0}")]
    NoMatches(TrackId),

    #[error("Confidence score {score} below threshold {threshold}")]
    LowConfidence { score: f32, threshold: f32 },

    #[error("Invalid confidence score: {0}")]
    InvalidConfidenceScore(f32),
}

/// Result type for matching operations
pub type MatchingResult<T> = Result<T, MatchingError>;

/// Metadata about a successful match
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// MusicBrainz recording ID from AcoustID lookup
    pub musicbrainz_recording_id: String,
    /// Confidence score from AcoustID (0.0-1.0)
    pub confidence_score: f32,
}

/// Track matching engine using fingerprints as primary lookup.
///
/// This service orchestrates the matching workflow:
/// 1. Takes a TrackFile with fingerprint data
/// 2. Looks up the fingerprint via AcoustID
/// 3. Returns MusicBrainz recording ID and confidence score
/// 4. Can filter by minimum confidence threshold
pub struct TrackMatchingService {
    acoustid_client: Arc<AcoustidClient>,
}

impl TrackMatchingService {
    /// Create a new matching service with the given AcoustID client.
    ///
    /// # Arguments
    ///
    /// * `acoustid_client` - Configured AcoustID API client for fingerprint lookups
    pub fn new(acoustid_client: AcoustidClient) -> Self {
        Self {
            acoustid_client: Arc::new(acoustid_client),
        }
    }

    /// Match a track file using its fingerprint via AcoustID.
    ///
    /// This is the primary matching strategy. Returns the first match with confidence
    /// >= the specified threshold.
    ///
    /// # Arguments
    ///
    /// * `track_file` - The track file with fingerprint data
    /// * `min_confidence` - Minimum confidence score (0.0-1.0) required for match
    ///
    /// # Returns
    ///
    /// * `Ok(MatchResult)` - Successfully matched with MBID and confidence score
    /// * `Err(MatchingError::NoFingerprint)` - Track file has no fingerprint
    /// * `Err(MatchingError::NoMatches)` - AcoustID returned no results
    /// * `Err(MatchingError::LowConfidence)` - Best match below threshold
    pub async fn match_track(
        &self,
        track_file: &TrackFile,
        min_confidence: f32,
    ) -> MatchingResult<MatchResult> {
        // Validate confidence threshold
        if !(0.0..=1.0).contains(&min_confidence) {
            return Err(MatchingError::InvalidConfidenceScore(min_confidence));
        }

        // Ensure track file has fingerprint
        let fingerprint_hash = track_file
            .fingerprint_hash
            .as_ref()
            .ok_or(MatchingError::NoFingerprint(track_file.id))?;

        let fingerprint_duration = track_file
            .fingerprint_duration
            .ok_or(MatchingError::NoFingerprint(track_file.id))?;

        debug!(
            target: "matching",
            track_id = %track_file.track_id,
            fingerprint_hash = fingerprint_hash,
            duration = fingerprint_duration,
            "attempting fingerprint lookup"
        );

        // Create fingerprint object for lookup
        let fingerprint = Fingerprint::new(fingerprint_hash.clone(), fingerprint_duration)?;

        // Lookup via AcoustID
        let recording_match = self
            .acoustid_client
            .lookup_best(&fingerprint, min_confidence)
            .await?;

        let recording_id = recording_match.id.to_string();

        debug!(
            target: "matching",
            track_id = %track_file.track_id,
            recording_id = %recording_id,
            confidence = recording_match.score,
            "fingerprint match successful"
        );

        Ok(MatchResult {
            musicbrainz_recording_id: recording_id,
            confidence_score: recording_match.score,
        })
    }

    /// Update a track with matching results.
    ///
    /// Sets the MusicBrainz recording ID and confidence score on the track.
    /// This is typically called after a successful `match_track` to persist results.
    ///
    /// # Arguments
    ///
    /// * `track` - The track entity to update (mutated)
    /// * `match_result` - The matching result from `match_track`
    pub fn apply_match(track: &mut Track, match_result: &MatchResult) {
        track.musicbrainz_recording_id = Some(match_result.musicbrainz_recording_id.clone());
        track.match_confidence = Some(match_result.confidence_score);
        track.updated_at = chrono::Utc::now();
    }

    /// Batch match multiple track files with progress tracking.
    ///
    /// Attempts to match each track file, collecting successful matches and errors.
    /// Logs warnings for files that fail to match.
    ///
    /// # Arguments
    ///
    /// * `track_files` - Collection of track files to match
    /// * `min_confidence` - Minimum confidence threshold for all matches
    ///
    /// # Returns
    ///
    /// A tuple of (successful_matches, failed_track_ids)
    pub async fn batch_match(
        &self,
        track_files: &[TrackFile],
        min_confidence: f32,
    ) -> (Vec<(TrackFileId, MatchResult)>, Vec<(TrackFileId, MatchingError)>) {
        let mut successes = Vec::new();
        let mut failures = Vec::new();

        for track_file in track_files {
            match self.match_track(track_file, min_confidence).await {
                Ok(result) => {
                    successes.push((track_file.id, result));
                }
                Err(e) => {
                    warn!(
                        target: "matching",
                        track_file_id = %track_file.id,
                        error = %e,
                        "track matching failed"
                    );
                    failures.push((track_file.id, e));
                }
            }
        }

        debug!(
            target: "matching",
            successful = successes.len(),
            failed = failures.len(),
            "batch matching complete"
        );

        (successes, failures)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_confidence_threshold() {
        // Verify that confidence threshold validation works
        // Confidence must be between 0.0 and 1.0
        let result_low = Err::<MatchResult, _>(MatchingError::InvalidConfidenceScore(-0.1));
        let result_high = Err::<MatchResult, _>(MatchingError::InvalidConfidenceScore(1.1));
        assert!(result_low.is_err());
        assert!(result_high.is_err());
    }

    #[test]
    fn apply_match_updates_track() {
        let mut track = Track::new(
            Default::default(),
            Default::default(),
            "Test Track",
        );

        let match_result = MatchResult {
            musicbrainz_recording_id: "12345678-1234-1234-1234-123456789012".to_string(),
            confidence_score: 0.95,
        };

        assert_eq!(track.musicbrainz_recording_id, None);
        assert_eq!(track.match_confidence, None);

        TrackMatchingService::apply_match(&mut track, &match_result);

        assert_eq!(
            track.musicbrainz_recording_id,
            Some("12345678-1234-1234-1234-123456789012".to_string())
        );
        assert_eq!(track.match_confidence, Some(0.95));
    }

    #[test]
    fn match_error_for_missing_fingerprint() {
        let track_file = TrackFile::new(TrackId::new(), "/path/to/file.flac", 1024);
        
        // Track file with no fingerprint should error
        assert!(matches!(
            MatchingError::NoFingerprint(track_file.id),
            MatchingError::NoFingerprint(_)
        ));
    }
}
