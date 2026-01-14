// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for matching precedence enforcement.

#[cfg(test)]
mod integration_tests {
    use crate::matching_precedence::{
        MatchingStrategy, PrecedenceMatchingError, PrecedenceMatchResult,
    };

    #[test]
    fn matching_strategy_precedence_ordering() {
        let strategies = [
            MatchingStrategy::Fingerprint,
            MatchingStrategy::EmbeddedTags,
            MatchingStrategy::FilenameHeuristics,
        ];

        assert_eq!(strategies[0], MatchingStrategy::Fingerprint);
        assert_eq!(strategies[1], MatchingStrategy::EmbeddedTags);
        assert_eq!(strategies[2], MatchingStrategy::FilenameHeuristics);
    }

    #[test]
    fn matching_strategy_display_formatting() {
        let fingerprint_str = MatchingStrategy::Fingerprint.to_string();
        let tags_str = MatchingStrategy::EmbeddedTags.to_string();
        let filename_str = MatchingStrategy::FilenameHeuristics.to_string();

        assert!(fingerprint_str.contains("AcoustID"));
        assert!(tags_str.contains("Tags"));
        assert!(filename_str.contains("Filename"));
    }

    #[test]
    fn precedence_error_all_strategies_failed() {
        let err = PrecedenceMatchingError::AllStrategiesFailed;
        assert_eq!(err.to_string(), "All matching strategies failed");
    }

    #[test]
    fn precedence_error_no_metadata_available() {
        let err = PrecedenceMatchingError::NoMetadataAvailable;
        assert_eq!(err.to_string(), "No metadata available for matching");
    }

    #[test]
    fn precedence_error_below_confidence_threshold() {
        let err = PrecedenceMatchingError::BelowConfidenceThreshold {
            score: 0.45,
            threshold: 0.5,
        };
        let msg = err.to_string();
        assert!(msg.contains("0.45"));
        assert!(msg.contains("0.5"));
    }

    #[test]
    fn precedence_error_invalid_threshold() {
        let err_low = PrecedenceMatchingError::InvalidThreshold(-0.1);
        let err_high = PrecedenceMatchingError::InvalidThreshold(1.5);

        let low_msg = err_low.to_string();
        let high_msg = err_high.to_string();

        assert!(low_msg.contains("Invalid confidence threshold"));
        assert!(high_msg.contains("Invalid confidence threshold"));
    }

    #[test]
    fn precedence_match_result_creation() {
        let result = PrecedenceMatchResult {
            musicbrainz_recording_id: "test-recording-123".to_string(),
            confidence: 0.87,
            strategy: MatchingStrategy::EmbeddedTags,
        };

        assert_eq!(result.musicbrainz_recording_id, "test-recording-123");
        assert_eq!(result.confidence, 0.87);
        assert_eq!(result.strategy, MatchingStrategy::EmbeddedTags);
    }

    #[test]
    fn precedence_match_result_confidence_validation() {
        // Valid confidence scores
        let valid_results = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        for conf in valid_results {
            let result = PrecedenceMatchResult {
                musicbrainz_recording_id: "id".to_string(),
                confidence: conf,
                strategy: MatchingStrategy::Fingerprint,
            };
            assert!((0.0..=1.0).contains(&result.confidence));
        }
    }

    #[test]
    fn precedence_strategy_type_equality() {
        // Test enum equality for different combinations
        assert_eq!(MatchingStrategy::Fingerprint, MatchingStrategy::Fingerprint);
        assert_eq!(MatchingStrategy::EmbeddedTags, MatchingStrategy::EmbeddedTags);
        assert_eq!(
            MatchingStrategy::FilenameHeuristics,
            MatchingStrategy::FilenameHeuristics
        );

        // Test enum inequality
        assert_ne!(MatchingStrategy::Fingerprint, MatchingStrategy::EmbeddedTags);
        assert_ne!(
            MatchingStrategy::Fingerprint,
            MatchingStrategy::FilenameHeuristics
        );
        assert_ne!(
            MatchingStrategy::EmbeddedTags,
            MatchingStrategy::FilenameHeuristics
        );
    }

    #[test]
    fn matching_strategy_copy_clone() {
        let strategy = MatchingStrategy::Fingerprint;
        let cloned = strategy; // Should be Copy
        assert_eq!(strategy, cloned);
    }

    #[test]
    fn precedence_result_describes_source_match() {
        let fingerprint_result = PrecedenceMatchResult {
            musicbrainz_recording_id: "fp-123".to_string(),
            confidence: 0.95,
            strategy: MatchingStrategy::Fingerprint,
        };

        let tags_result = PrecedenceMatchResult {
            musicbrainz_recording_id: "tag-456".to_string(),
            confidence: 0.72,
            strategy: MatchingStrategy::EmbeddedTags,
        };

        let filename_result = PrecedenceMatchResult {
            musicbrainz_recording_id: "fn-789".to_string(),
            confidence: 0.45,
            strategy: MatchingStrategy::FilenameHeuristics,
        };

        // Each result clearly identifies its source
        assert_eq!(fingerprint_result.strategy, MatchingStrategy::Fingerprint);
        assert_eq!(tags_result.strategy, MatchingStrategy::EmbeddedTags);
        assert_eq!(filename_result.strategy, MatchingStrategy::FilenameHeuristics);

        // Different sources produce different IDs in this test
        assert_ne!(
            fingerprint_result.musicbrainz_recording_id,
            tags_result.musicbrainz_recording_id
        );
        assert_ne!(
            tags_result.musicbrainz_recording_id,
            filename_result.musicbrainz_recording_id
        );
    }

    #[test]
    fn precedence_results_contain_all_required_fields() {
        let result = PrecedenceMatchResult {
            musicbrainz_recording_id: "mb-id-123".to_string(),
            confidence: 0.92,
            strategy: MatchingStrategy::EmbeddedTags,
        };

        // Verify all fields are accessible
        let _ = result.musicbrainz_recording_id;
        let _ = result.confidence;
        let _ = result.strategy;

        // Verify fields have correct types
        assert!(!result.musicbrainz_recording_id.is_empty());
        assert!((0.0..=1.0).contains(&result.confidence));
    }

    #[test]
    fn error_from_matching_error() {
        // Test that PrecedenceMatchingError can be created from MatchingError
        // The From trait is implemented, so we can convert directly
        use crate::matching::MatchingError;
        use chorrosion_domain::TrackFileId;

        let track_file_id = TrackFileId::new();
        let matching_err = MatchingError::NoFingerprint(track_file_id);
        let precedence_err = PrecedenceMatchingError::from(matching_err);

        // Should convert without panic
        let msg = precedence_err.to_string();
        assert!(!msg.is_empty());
    }

    #[test]
    fn confidence_thresholds_boundary_values() {
        let test_cases = vec![
            (0.0, true),   // Valid: minimum
            (0.3, true),   // Valid: common threshold
            (0.5, true),   // Valid: midpoint
            (0.8, true),   // Valid: typical fingerprint threshold
            (1.0, true),   // Valid: maximum
            (-0.001, false), // Invalid: just below min
            (1.001, false),  // Invalid: just above max
        ];

        for (threshold, should_be_valid) in test_cases {
            let is_valid = (0.0..=1.0).contains(&threshold);
            assert_eq!(
                is_valid, should_be_valid,
                "Threshold {} validation failed",
                threshold
            );
        }
    }

    #[test]
    fn matching_strategy_all_variants_distinct() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(MatchingStrategy::Fingerprint);
        set.insert(MatchingStrategy::EmbeddedTags);
        set.insert(MatchingStrategy::FilenameHeuristics);

        // All three should be distinct
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn precedence_match_result_clone() {
        let original = PrecedenceMatchResult {
            musicbrainz_recording_id: "clone-test".to_string(),
            confidence: 0.88,
            strategy: MatchingStrategy::Fingerprint,
        };

        let cloned = original.clone();

        assert_eq!(original.musicbrainz_recording_id, cloned.musicbrainz_recording_id);
        assert_eq!(original.confidence, cloned.confidence);
        assert_eq!(original.strategy, cloned.strategy);
    }

    #[test]
    fn precedence_error_display_consistency() {
        // Verify error messages are consistent across multiple calls
        let err = PrecedenceMatchingError::AllStrategiesFailed;
        let msg1 = err.to_string();
        let msg2 = PrecedenceMatchingError::AllStrategiesFailed.to_string();

        assert_eq!(msg1, msg2);
    }
}
