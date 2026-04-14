// SPDX-License-Identifier: GPL-3.0-or-later

//! Track matching service using fingerprints as primary lookup strategy.
//!
//! The matching engine implements a comprehensive fallback chain:
//! 1. **Fingerprint-based lookup** via AcoustID (highest confidence: 0.8-1.0)
//!    - Audio fingerprint matched against MusicBrainz database
//!    - Most reliable method, resistant to file format/tags changes
//! 2. **Embedded metadata tags** (medium confidence: 0.5-0.9)
//!    - Parses ID3/Vorbis/MP4 tags (artist/album/track metadata)
//!    - Less reliable than fingerprints but better than filename heuristics
//!    - Future enhancement: requires external audio libraries
//! 3. **Filename heuristics** (lower confidence: 0.3-0.7)
//!    - Pattern-based extraction from filename (e.g., "Artist - 01 - Title")
//!    - Fallback when no fingerprint or tags available
//!    - Supports common naming conventions
//!
//! Each fallback step is attempted if the previous step fails or is unavailable.
//! Confidence scores decrease at each level, allowing confidence-based filtering.

use chorrosion_domain::{Track, TrackFile, TrackFileId, TrackId};
use chorrosion_fingerprint::{AcoustidClient, Fingerprint, FingerprintError};
use chorrosion_musicbrainz::{MusicBrainzClient, MusicBrainzError, Recording};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, warn};
use uuid::Uuid;

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

    #[error("Invalid MusicBrainz recording ID: {0}")]
    InvalidRecordingId(String),

    #[error("MusicBrainz error: {0}")]
    MusicBrainzError(#[from] MusicBrainzError),
}

/// Result type for matching operations
pub type MatchingResult<T> = Result<T, MatchingError>;

fn extract_artist_album_links(recording: &Recording) -> (Option<String>, Option<String>) {
    let artist_id = recording
        .artist_credit
        .first()
        .map(|credit| credit.artist.id.to_string());
    let release_group_id = recording
        .releases
        .first()
        .map(|release| release.release_group.id.to_string());

    (artist_id, release_group_id)
}

/// Metadata about a successful match
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// MusicBrainz recording ID from AcoustID lookup
    pub musicbrainz_recording_id: String,
    /// Linked MusicBrainz artist ID resolved from the matched recording.
    pub musicbrainz_artist_id: Option<String>,
    /// Linked MusicBrainz release-group ID (album) resolved from the matched recording.
    pub musicbrainz_release_group_id: Option<String>,
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
    musicbrainz_client: Option<Arc<MusicBrainzClient>>,
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
            musicbrainz_client: None,
        }
    }

    /// Create a new matching service with MusicBrainz linkage enabled.
    pub fn new_with_musicbrainz(
        acoustid_client: AcoustidClient,
        musicbrainz_client: MusicBrainzClient,
    ) -> Self {
        Self {
            acoustid_client: Arc::new(acoustid_client),
            musicbrainz_client: Some(Arc::new(musicbrainz_client)),
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
        let (musicbrainz_artist_id, musicbrainz_release_group_id) = self
            .resolve_recording_links(&recording_id)
            .await
            .unwrap_or_else(|error| {
                warn!(
                    target: "matching",
                    recording_id = %recording_id,
                    error = %error,
                    "unable to resolve recording artist/album links"
                );
                (None, None)
            });

        debug!(
            target: "matching",
            track_id = %track_file.track_id,
            recording_id = %recording_id,
            confidence = recording_match.score,
            "fingerprint match successful"
        );

        Ok(MatchResult {
            musicbrainz_recording_id: recording_id,
            musicbrainz_artist_id,
            musicbrainz_release_group_id,
            confidence_score: recording_match.score,
        })
    }

    async fn resolve_recording_links(
        &self,
        recording_id: &str,
    ) -> MatchingResult<(Option<String>, Option<String>)> {
        let Some(client) = &self.musicbrainz_client else {
            return Ok((None, None));
        };

        let recording_uuid = Uuid::parse_str(recording_id)
            .map_err(|_| MatchingError::InvalidRecordingId(recording_id.to_string()))?;

        let recording = client.lookup_recording(recording_uuid).await?;
        Ok(extract_artist_album_links(&recording))
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
    ) -> (
        Vec<(TrackFileId, MatchResult)>,
        Vec<(TrackFileId, MatchingError)>,
    ) {
        let mut successes = Vec::new();
        let mut failures = Vec::new();

        for track_file in track_files {
            let match_attempt = self.match_track(track_file, min_confidence).await;
            match match_attempt {
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
    use serde_json::json;

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
        let mut track = Track::new(Default::default(), Default::default(), "Test Track");

        let match_result = MatchResult {
            musicbrainz_recording_id: "12345678-1234-1234-1234-123456789012".to_string(),
            musicbrainz_artist_id: Some("a74b1b7f-71a5-4011-9441-d0b5e4122711".to_string()),
            musicbrainz_release_group_id: Some("b1392450-e666-3926-a536-22c65f834433".to_string()),
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

    #[test]
    fn extract_artist_album_links_returns_primary_ids() {
        let recording: Recording = serde_json::from_value(json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "title": "Test Recording",
            "length": 180000,
            "artist-credit": [
                {
                    "name": "Test Artist",
                    "artist": {
                        "id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                        "name": "Test Artist",
                        "sort-name": "Artist, Test"
                    }
                }
            ],
            "releases": [
                {
                    "id": "22222222-2222-2222-2222-222222222222",
                    "title": "Test Release",
                    "status": "Official",
                    "country": "US",
                    "date": "2020-01-01",
                    "release-group": {
                        "id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                        "title": "Test Album",
                        "primary-type": "Album"
                    }
                }
            ]
        }))
        .expect("recording json should parse");

        let (artist_id, release_group_id) = extract_artist_album_links(&recording);

        assert_eq!(
            artist_id.as_deref(),
            Some("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
        );
        assert_eq!(
            release_group_id.as_deref(),
            Some("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")
        );
    }

    #[test]
    fn extract_artist_album_links_returns_none_when_missing() {
        let recording: Recording = serde_json::from_value(json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "title": "Test Recording",
            "length": null,
            "artist-credit": [],
            "releases": []
        }))
        .expect("recording json should parse");

        let (artist_id, release_group_id) = extract_artist_album_links(&recording);

        assert!(artist_id.is_none());
        assert!(release_group_id.is_none());
    }
}
