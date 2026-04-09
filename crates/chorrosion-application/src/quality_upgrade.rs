// SPDX-License-Identifier: GPL-3.0-or-later
//! Quality upgrade evaluation for the file management pipeline.
//!
//! This module provides logic for determining whether a newly found audio file
//! represents a quality upgrade over an existing `TrackFile`, based on the
//! artist's [`QualityProfile`].
//!
//! # Quality ordering
//!
//! Qualities are ranked by their position in
//! `QualityProfile::allowed_qualities`: index 0 is the lowest quality and
//! higher indices are progressively better.  For example:
//!
//! ```text
//! allowed_qualities = ["MP3 128", "MP3 256", "MP3 320", "FLAC"]
//! ```
//!
//! would rank them 0 → 1 → 2 → 3.
//!
//! # Upgrade policy
//!
//! | Existing quality | Upgrade allowed? | Decision |
//! |---|---|---|
//! | Below `cutoff_quality` | always | `Upgrade(BelowCutoff)` |
//! | At/above `cutoff_quality` and `upgrade_allowed = true` | yes | `Upgrade(BetterQualityAvailable)` |
//! | At/above `cutoff_quality` and `upgrade_allowed = false` | no | `Keep` |
//! | Candidate is not better | — | `Keep` |

use chorrosion_domain::QualityProfile;

// ============================================================================
// Quality comparison helpers
// ============================================================================

/// Quality ranking and comparison utilities based on a [`QualityProfile`].
pub struct QualityComparer;

impl QualityComparer {
    /// Returns the rank of `quality` within `profile.allowed_qualities` (0 =
    /// lowest), or `None` if the quality is not in the profile's list.
    pub fn rank(quality: &str, profile: &QualityProfile) -> Option<usize> {
        profile
            .allowed_qualities
            .iter()
            .position(|q| q.as_str() == quality)
    }

    /// Returns `true` if `candidate` is strictly ranked higher than `existing`
    /// within the profile.  Returns `false` if either quality is unknown.
    pub fn is_upgrade(existing: &str, candidate: &str, profile: &QualityProfile) -> bool {
        match (
            Self::rank(existing, profile),
            Self::rank(candidate, profile),
        ) {
            (Some(e), Some(c)) => c > e,
            _ => false,
        }
    }

    /// Returns `true` if `quality` meets or exceeds the profile's
    /// `cutoff_quality`.  When no cutoff is configured, any allowed quality
    /// passes.
    pub fn meets_cutoff(quality: &str, profile: &QualityProfile) -> bool {
        let Some(cutoff) = &profile.cutoff_quality else {
            // No cutoff configured — any quality in the allowed list passes.
            return Self::rank(quality, profile).is_some();
        };
        match (Self::rank(quality, profile), Self::rank(cutoff, profile)) {
            (Some(q), Some(c)) => q >= c,
            _ => false,
        }
    }
}

// ============================================================================
// Upgrade decision
// ============================================================================

/// The reason an upgrade was approved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeReason {
    /// The existing file's quality is below the profile's cutoff; an upgrade
    /// is always attempted regardless of `upgrade_allowed`.
    BelowCutoff,
    /// The existing file meets the cutoff but `upgrade_allowed = true` and
    /// the candidate is of higher quality.
    BetterQualityAvailable,
}

/// The outcome of evaluating a potential quality upgrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeDecision {
    /// The existing file should be kept; the candidate is not an improvement.
    Keep,
    /// The existing file should be replaced with the candidate.
    Upgrade {
        /// Why the upgrade was approved.
        reason: UpgradeReason,
    },
}

// ============================================================================
// Quality upgrade service
// ============================================================================

/// Evaluates whether a candidate audio file is a quality upgrade over an
/// existing [`TrackFile`] according to the artist's [`QualityProfile`].
pub struct QualityUpgradeService;

impl QualityUpgradeService {
    /// Evaluate an upgrade decision.
    ///
    /// # Arguments
    ///
    /// * `existing_quality` — the quality label of the on-disk file (e.g.
    ///   `"MP3 320"`).
    /// * `candidate_quality` — the quality label of the new candidate file.
    /// * `profile` — the [`QualityProfile`] governing upgrade decisions.
    ///
    /// # Returns
    ///
    /// [`UpgradeDecision::Upgrade`] when the candidate should replace the
    /// existing file, [`UpgradeDecision::Keep`] otherwise.
    pub fn evaluate_upgrade(
        existing_quality: &str,
        candidate_quality: &str,
        profile: &QualityProfile,
    ) -> UpgradeDecision {
        // The candidate must be strictly better; otherwise there is nothing to
        // gain from replacing the file.
        if !QualityComparer::is_upgrade(existing_quality, candidate_quality, profile) {
            return UpgradeDecision::Keep;
        }

        // Always upgrade when the existing file is below the cutoff.
        if !QualityComparer::meets_cutoff(existing_quality, profile) {
            return UpgradeDecision::Upgrade {
                reason: UpgradeReason::BelowCutoff,
            };
        }

        // Existing file meets or exceeds the cutoff.  Only upgrade if the
        // profile explicitly allows continued upgrades.
        if profile.upgrade_allowed {
            UpgradeDecision::Upgrade {
                reason: UpgradeReason::BetterQualityAvailable,
            }
        } else {
            UpgradeDecision::Keep
        }
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chorrosion_domain::{ProfileId, QualityProfile};
    use chrono::Utc;

    fn make_profile(
        allowed: &[&str],
        cutoff: Option<&str>,
        upgrade_allowed: bool,
    ) -> QualityProfile {
        QualityProfile {
            id: ProfileId::new(),
            name: "test".to_string(),
            allowed_qualities: allowed.iter().map(|s| s.to_string()).collect(),
            upgrade_allowed,
            cutoff_quality: cutoff.map(str::to_string),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ---- QualityComparer::rank ----

    #[test]
    fn rank_returns_correct_index() {
        let profile = make_profile(&["MP3 128", "MP3 320", "FLAC"], None, false);
        assert_eq!(QualityComparer::rank("MP3 128", &profile), Some(0));
        assert_eq!(QualityComparer::rank("MP3 320", &profile), Some(1));
        assert_eq!(QualityComparer::rank("FLAC", &profile), Some(2));
    }

    #[test]
    fn rank_returns_none_for_unknown_quality() {
        let profile = make_profile(&["FLAC"], None, false);
        assert_eq!(QualityComparer::rank("OGG 192", &profile), None);
    }

    // ---- QualityComparer::is_upgrade ----

    #[test]
    fn is_upgrade_true_when_candidate_ranked_higher() {
        let profile = make_profile(&["MP3 128", "FLAC"], None, false);
        assert!(QualityComparer::is_upgrade("MP3 128", "FLAC", &profile));
    }

    #[test]
    fn is_upgrade_false_when_candidate_same_or_lower() {
        let profile = make_profile(&["MP3 128", "FLAC"], None, false);
        assert!(!QualityComparer::is_upgrade("FLAC", "MP3 128", &profile));
        assert!(!QualityComparer::is_upgrade("FLAC", "FLAC", &profile));
    }

    #[test]
    fn is_upgrade_false_for_unknown_qualities() {
        let profile = make_profile(&["FLAC"], None, false);
        // existing unknown
        assert!(!QualityComparer::is_upgrade("WAV", "FLAC", &profile));
        // candidate unknown
        assert!(!QualityComparer::is_upgrade("FLAC", "WAV", &profile));
    }

    // ---- QualityComparer::meets_cutoff ----

    #[test]
    fn meets_cutoff_true_when_at_or_above_cutoff() {
        let profile = make_profile(&["MP3 128", "MP3 320", "FLAC"], Some("MP3 320"), false);
        assert!(QualityComparer::meets_cutoff("MP3 320", &profile));
        assert!(QualityComparer::meets_cutoff("FLAC", &profile));
    }

    #[test]
    fn meets_cutoff_false_when_below_cutoff() {
        let profile = make_profile(&["MP3 128", "MP3 320", "FLAC"], Some("MP3 320"), false);
        assert!(!QualityComparer::meets_cutoff("MP3 128", &profile));
    }

    #[test]
    fn meets_cutoff_true_when_no_cutoff_and_quality_in_list() {
        let profile = make_profile(&["MP3 128", "FLAC"], None, false);
        assert!(QualityComparer::meets_cutoff("MP3 128", &profile));
        assert!(QualityComparer::meets_cutoff("FLAC", &profile));
    }

    #[test]
    fn meets_cutoff_false_for_unknown_quality() {
        let profile = make_profile(&["FLAC"], Some("FLAC"), false);
        assert!(!QualityComparer::meets_cutoff("OGG 192", &profile));
    }

    // ---- QualityUpgradeService::evaluate_upgrade ----

    #[test]
    fn upgrade_approved_when_existing_below_cutoff() {
        let profile = make_profile(&["MP3 128", "MP3 320", "FLAC"], Some("MP3 320"), false);
        let decision = QualityUpgradeService::evaluate_upgrade("MP3 128", "MP3 320", &profile);
        assert_eq!(
            decision,
            UpgradeDecision::Upgrade {
                reason: UpgradeReason::BelowCutoff
            }
        );
    }

    #[test]
    fn upgrade_approved_when_above_cutoff_and_upgrade_allowed() {
        let profile = make_profile(&["MP3 128", "MP3 320", "FLAC"], Some("MP3 320"), true);
        let decision = QualityUpgradeService::evaluate_upgrade("MP3 320", "FLAC", &profile);
        assert_eq!(
            decision,
            UpgradeDecision::Upgrade {
                reason: UpgradeReason::BetterQualityAvailable
            }
        );
    }

    #[test]
    fn upgrade_denied_when_above_cutoff_and_upgrade_not_allowed() {
        let profile = make_profile(&["MP3 128", "MP3 320", "FLAC"], Some("MP3 320"), false);
        let decision = QualityUpgradeService::evaluate_upgrade("MP3 320", "FLAC", &profile);
        assert_eq!(decision, UpgradeDecision::Keep);
    }

    #[test]
    fn upgrade_denied_when_candidate_not_better() {
        let profile = make_profile(&["MP3 128", "MP3 320", "FLAC"], Some("MP3 320"), true);
        // Same quality
        assert_eq!(
            QualityUpgradeService::evaluate_upgrade("FLAC", "FLAC", &profile),
            UpgradeDecision::Keep
        );
        // Downgrade
        assert_eq!(
            QualityUpgradeService::evaluate_upgrade("FLAC", "MP3 128", &profile),
            UpgradeDecision::Keep
        );
    }

    #[test]
    fn upgrade_denied_for_unknown_qualities() {
        let profile = make_profile(&["FLAC"], Some("FLAC"), true);
        assert_eq!(
            QualityUpgradeService::evaluate_upgrade("WAV", "FLAC", &profile),
            UpgradeDecision::Keep
        );
    }
}
