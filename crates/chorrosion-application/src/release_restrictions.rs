// SPDX-License-Identifier: GPL-3.0-or-later
//! Release restriction rules and evaluation.
//!
//! This module provides a rule-based system for filtering out unwanted releases
//! based on metadata like country, label, format combinations, and more.
//!
//! # Restriction Rules
//!
//! Restrictions are defined as rules that, when matched, cause a release to be
//! excluded from consideration. Each rule type targets specific release metadata:
//!
//! - **Country**: Exclude releases from specific countries (e.g., "JP" for Japan)
//! - **Label**: Exclude releases from specific labels (e.g., "Sony", "Universal")
//! - **Format combination**: Exclude specific combinations (e.g., "MP3 + 128kbps")
//! - **Release group**: Exclude specific release groups by name
//! - **Keyword**: Exclude releases containing specific keywords (case-insensitive)
//!
//! # Evaluation
//!
//! A release is restricted if it matches any rule in the restriction set.
//! Use [`ReleaseRestrictionSet::is_restricted()`] to test a release.

use crate::release_parsing::{AudioQuality, ParsedReleaseTitle};
use serde::{Deserialize, Serialize};

/// A single restriction rule that can exclude a release from consideration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RestrictionRule {
    /// Exclude releases from a specific country (ISO 3166-1 alpha-2 code).
    /// Example: { "type": "Country", "code": "JP" }
    Country { code: String },

    /// Exclude releases from a specific label.
    /// Example: { "type": "Label", "name": "Sony" }
    Label { name: String },

    /// Exclude releases matching a specific format and bitrate.
    /// Example: { "type": "FormatCombination", "quality": "mp3", "bitrate_kbps": 128 }
    FormatCombination {
        quality: AudioQuality,
        bitrate_kbps: Option<u32>,
    },

    /// Exclude releases from a specific release group.
    /// Example: { "type": "ReleaseGroup", "name": "BadGroup" }
    ReleaseGroup { name: String },

    /// Exclude releases containing a specific keyword (case-insensitive).
    /// Example: { "type": "Keyword", "keyword": "remix" }
    Keyword { keyword: String },
}

impl RestrictionRule {
    /// Check if this rule matches the given release.
    ///
    /// # Arguments
    ///
    /// * `release` — the parsed release title to check
    /// * `country` — optional country code for the release (if available)
    /// * `label` — optional label name for the release (if available)
    ///
    /// # Returns
    ///
    /// `true` if the release matches this rule (and should be restricted).
    pub fn matches(
        &self,
        release: &ParsedReleaseTitle,
        country: Option<&str>,
        label: Option<&str>,
    ) -> bool {
        match self {
            RestrictionRule::Country { code } => {
                country.is_some_and(|c| c.eq_ignore_ascii_case(code))
            }
            RestrictionRule::Label { name } => label.is_some_and(|l| l.eq_ignore_ascii_case(name)),
            RestrictionRule::FormatCombination {
                quality,
                bitrate_kbps,
            } => {
                if release.quality != *quality {
                    return false;
                }
                match bitrate_kbps {
                    Some(required_bitrate) => release.bitrate_kbps == Some(*required_bitrate),
                    None => true, // No bitrate specified — any bitrate for this quality matches
                }
            }
            RestrictionRule::ReleaseGroup { name } => release
                .release_group
                .as_deref()
                .is_some_and(|g| g.eq_ignore_ascii_case(name)),
            RestrictionRule::Keyword { keyword } => {
                contains_ignore_ascii_case(&release.original_title, keyword)
            }
        }
    }
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    let needle_bytes = needle.as_bytes();
    if needle_bytes.is_empty() {
        return true;
    }

    haystack
        .as_bytes()
        .windows(needle_bytes.len())
        .any(|window| window.eq_ignore_ascii_case(needle_bytes))
}

/// A set of release restriction rules.
///
/// When evaluating a release, it is considered restricted if it matches
/// any rule in the set.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReleaseRestrictionSet {
    pub rules: Vec<RestrictionRule>,
}

impl ReleaseRestrictionSet {
    /// Create an empty restriction set.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Create a restriction set from a list of rules.
    pub fn with_rules(rules: Vec<RestrictionRule>) -> Self {
        Self { rules }
    }

    /// Add a rule to the set.
    pub fn add_rule(&mut self, rule: RestrictionRule) {
        self.rules.push(rule);
    }

    /// Check if a release is restricted by any rule in this set.
    ///
    /// A release is restricted if it matches any rule. Returns `true`
    /// if the release should be excluded.
    pub fn is_restricted(
        &self,
        release: &ParsedReleaseTitle,
        country: Option<&str>,
        label: Option<&str>,
    ) -> bool {
        self.rules
            .iter()
            .any(|rule| rule.matches(release, country, label))
    }

    /// Check if a release is allowed (not restricted by any rule).
    ///
    /// This is the inverse of [`is_restricted()`](Self::is_restricted).
    pub fn is_allowed(
        &self,
        release: &ParsedReleaseTitle,
        country: Option<&str>,
        label: Option<&str>,
    ) -> bool {
        !self.is_restricted(release, country, label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release_parsing::ParsedReleaseTitle;

    fn make_release(
        title: &str,
        artist: Option<&str>,
        album: Option<&str>,
        quality: AudioQuality,
        bitrate: Option<u32>,
        group: Option<&str>,
    ) -> ParsedReleaseTitle {
        ParsedReleaseTitle {
            original_title: title.to_string(),
            artist: artist.map(|s| s.to_string()),
            album: album.map(|s| s.to_string()),
            quality,
            bitrate_kbps: bitrate,
            release_group: group.map(|s| s.to_string()),
        }
    }

    #[test]
    fn country_rule_matches_exact_country() {
        let rule = RestrictionRule::Country {
            code: "JP".to_string(),
        };
        let release = make_release(
            "Artist - Album",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );

        assert!(rule.matches(&release, Some("JP"), None));
        assert!(!rule.matches(&release, Some("US"), None));
        assert!(!rule.matches(&release, None, None));
    }

    #[test]
    fn country_rule_case_insensitive() {
        let rule = RestrictionRule::Country {
            code: "jp".to_string(),
        };
        let release = make_release(
            "Artist - Album",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );

        assert!(rule.matches(&release, Some("JP"), None));
        assert!(rule.matches(&release, Some("jp"), None));
        assert!(rule.matches(&release, Some("Jp"), None));
    }

    #[test]
    fn label_rule_matches_exact_label() {
        let rule = RestrictionRule::Label {
            name: "Sony".to_string(),
        };
        let release = make_release(
            "Artist - Album",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );

        assert!(rule.matches(&release, None, Some("Sony")));
        assert!(!rule.matches(&release, None, Some("Universal")));
        assert!(!rule.matches(&release, None, None));
    }

    #[test]
    fn label_rule_case_insensitive() {
        let rule = RestrictionRule::Label {
            name: "sony".to_string(),
        };
        let release = make_release(
            "Artist - Album",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );

        assert!(rule.matches(&release, None, Some("SONY")));
        assert!(rule.matches(&release, None, Some("Sony")));
    }

    #[test]
    fn format_combination_rule_with_bitrate() {
        let rule = RestrictionRule::FormatCombination {
            quality: AudioQuality::Mp3,
            bitrate_kbps: Some(128),
        };

        let low_bitrate = make_release(
            "Artist - Album 128kbps",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Mp3,
            Some(128),
            None,
        );
        let high_bitrate = make_release(
            "Artist - Album 320kbps",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Mp3,
            Some(320),
            None,
        );
        let flac = make_release(
            "Artist - Album [FLAC]",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );

        assert!(rule.matches(&low_bitrate, None, None));
        assert!(!rule.matches(&high_bitrate, None, None));
        assert!(!rule.matches(&flac, None, None));
    }

    #[test]
    fn format_combination_rule_without_bitrate() {
        let rule = RestrictionRule::FormatCombination {
            quality: AudioQuality::Mp3,
            bitrate_kbps: None,
        };

        let mp3_low = make_release(
            "Artist - Album 128kbps",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Mp3,
            Some(128),
            None,
        );
        let mp3_high = make_release(
            "Artist - Album 320kbps",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Mp3,
            Some(320),
            None,
        );
        let flac = make_release(
            "Artist - Album [FLAC]",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );

        // Both MP3 releases should match, regardless of bitrate
        assert!(rule.matches(&mp3_low, None, None));
        assert!(rule.matches(&mp3_high, None, None));
        assert!(!rule.matches(&flac, None, None));
    }

    #[test]
    fn release_group_rule_matches_exact_group() {
        let rule = RestrictionRule::ReleaseGroup {
            name: "BadGroup".to_string(),
        };
        let bad_group = make_release(
            "Artist - Album-BadGroup",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            Some("BadGroup"),
        );
        let good_group = make_release(
            "Artist - Album-GoodGroup",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            Some("GoodGroup"),
        );

        assert!(rule.matches(&bad_group, None, None));
        assert!(!rule.matches(&good_group, None, None));
    }

    #[test]
    fn release_group_rule_case_insensitive() {
        let rule = RestrictionRule::ReleaseGroup {
            name: "badgroup".to_string(),
        };
        let release = make_release(
            "Artist - Album-BadGroup",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            Some("BadGroup"),
        );

        assert!(rule.matches(&release, None, None));
    }

    #[test]
    fn keyword_rule_matches_case_insensitive() {
        let rule = RestrictionRule::Keyword {
            keyword: "remix".to_string(),
        };

        let remix_release = make_release(
            "Artist - Album (Remix Mix)",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );
        let original = make_release(
            "Artist - Album",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );

        assert!(rule.matches(&remix_release, None, None));
        assert!(!rule.matches(&original, None, None));
    }

    #[test]
    fn keyword_rule_case_insensitive() {
        let rule = RestrictionRule::Keyword {
            keyword: "REMIX".to_string(),
        };
        let release = make_release(
            "Artist - Album remix version",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );

        assert!(rule.matches(&release, None, None));
    }

    #[test]
    fn empty_restriction_set_allows_all() {
        let set = ReleaseRestrictionSet::new();
        let release = make_release(
            "Artist - Album",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );

        assert!(!set.is_restricted(&release, None, None));
        assert!(set.is_allowed(&release, None, None));
    }

    #[test]
    fn restriction_set_restricts_matching_release() {
        let mut set = ReleaseRestrictionSet::new();
        set.add_rule(RestrictionRule::Country {
            code: "JP".to_string(),
        });
        set.add_rule(RestrictionRule::Keyword {
            keyword: "remix".to_string(),
        });

        let jp_release = make_release(
            "Artist - Album",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );
        let remix_release = make_release(
            "Artist - Album (Remix)",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );
        let allowed = make_release(
            "Artist - Album",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );

        assert!(set.is_restricted(&jp_release, Some("JP"), None));
        assert!(set.is_restricted(&remix_release, None, None));
        assert!(!set.is_restricted(&allowed, Some("US"), None));
    }

    #[test]
    fn restriction_set_with_multiple_rules() {
        let rules = vec![
            RestrictionRule::Country {
                code: "JP".to_string(),
            },
            RestrictionRule::Label {
                name: "Sony".to_string(),
            },
            RestrictionRule::FormatCombination {
                quality: AudioQuality::Mp3,
                bitrate_kbps: Some(128),
            },
        ];
        let set = ReleaseRestrictionSet::with_rules(rules);

        // JP country should be restricted
        let jp = make_release(
            "Artist - Album",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );
        assert!(set.is_restricted(&jp, Some("JP"), None));

        // Sony label should be restricted
        let sony = make_release(
            "Artist - Album",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );
        assert!(set.is_restricted(&sony, None, Some("Sony")));

        // MP3 128kbps should be restricted
        let mp3_low = make_release(
            "Artist - Album 128kbps",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Mp3,
            Some(128),
            None,
        );
        assert!(set.is_restricted(&mp3_low, None, None));

        // FLAC from Universal should be allowed
        let flac_allowed = make_release(
            "Artist - Album [FLAC]",
            Some("Artist"),
            Some("Album"),
            AudioQuality::Flac,
            None,
            None,
        );
        assert!(!set.is_restricted(&flac_allowed, Some("US"), Some("Universal")));
    }
}
