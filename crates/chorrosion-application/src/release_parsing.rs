// SPDX-License-Identifier: GPL-3.0-or-later
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioQuality {
    Flac,
    Mp3,
    Aac,
    Alac,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedReleaseTitle {
    pub original_title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub quality: AudioQuality,
    pub bitrate_kbps: Option<u32>,
    pub release_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReleaseFilterOptions {
    pub preferred_qualities: Vec<AudioQuality>,
    pub min_bitrate_kbps: Option<u32>,
    pub preferred_release_groups: Vec<String>,
    pub preferred_words: Vec<String>,
    pub custom_format_rules: Vec<CustomFormatRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomFormatRule {
    pub name: String,
    pub keywords: Vec<String>,
    pub score_bonus: i32,
}

#[derive(Debug, Clone)]
struct NormalizedCustomFormatRule {
    keywords: Vec<String>,
    score_bonus: i64,
}

const SCORE_MIN: i64 = i32::MIN as i64;
const SCORE_MAX: i64 = i32::MAX as i64;

pub fn parse_release_title(title: &str) -> ParsedReleaseTitle {
    let normalized = normalize_whitespace(title);
    let quality = detect_quality(&normalized);
    let bitrate_kbps = detect_bitrate_kbps(&normalized, &quality);
    let release_group = detect_release_group(&normalized);
    let (artist, album) = extract_artist_album(&normalized);

    ParsedReleaseTitle {
        original_title: title.to_string(),
        artist,
        album,
        quality,
        bitrate_kbps,
        release_group,
    }
}

pub fn filter_releases(
    releases: &[ParsedReleaseTitle],
    options: &ReleaseFilterOptions,
) -> Vec<ParsedReleaseTitle> {
    releases
        .iter()
        .filter(|release| {
            if !options.preferred_qualities.is_empty()
                && !options.preferred_qualities.contains(&release.quality)
            {
                return false;
            }

            if let Some(min_bitrate) = options.min_bitrate_kbps {
                match (&release.quality, release.bitrate_kbps) {
                    // Treat lossless formats as always satisfying the bitrate requirement,
                    // even when `detect_bitrate_kbps` returns None.
                    (&AudioQuality::Flac | &AudioQuality::Alac, _) => {}
                    // For other formats, enforce the minimum bitrate if we have a value.
                    (_, Some(bitrate)) if bitrate >= min_bitrate => {}
                    _ => return false,
                }
            }

            true
        })
        .cloned()
        .collect()
}

pub fn rank_releases(
    mut releases: Vec<ParsedReleaseTitle>,
    options: &ReleaseFilterOptions,
) -> Vec<ParsedReleaseTitle> {
    let normalized_preferred_words = normalize_preferred_words(&options.preferred_words);
    let normalized_custom_rules = normalize_custom_format_rules(&options.custom_format_rules);
    releases.sort_by_cached_key(|release| {
        std::cmp::Reverse(score_release_with_words(
            release,
            options,
            &normalized_preferred_words,
            &normalized_custom_rules,
        ))
    });
    releases
}

pub fn deduplicate_releases(releases: &[ParsedReleaseTitle]) -> Vec<ParsedReleaseTitle> {
    let mut best_by_key: HashMap<String, ParsedReleaseTitle> = HashMap::new();
    let default_options = ReleaseFilterOptions::default();
    let normalized_default_words = normalize_preferred_words(&default_options.preferred_words);
    let normalized_default_custom_rules =
        normalize_custom_format_rules(&default_options.custom_format_rules);

    for release in releases {
        let key = duplicate_key(release);
        match best_by_key.get(&key) {
            Some(existing) => {
                let existing_score = score_release_with_words(
                    existing,
                    &default_options,
                    &normalized_default_words,
                    &normalized_default_custom_rules,
                );
                let candidate_score = score_release_with_words(
                    release,
                    &default_options,
                    &normalized_default_words,
                    &normalized_default_custom_rules,
                );
                if candidate_score > existing_score {
                    best_by_key.insert(key, release.clone());
                }
            }
            None => {
                best_by_key.insert(key, release.clone());
            }
        }
    }

    let mut deduped: Vec<ParsedReleaseTitle> = best_by_key.into_values().collect();
    deduped.sort_by_key(|release| release.original_title.to_lowercase());
    deduped
}

pub fn find_duplicate_keys(releases: &[ParsedReleaseTitle]) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for release in releases {
        let key = duplicate_key(release);
        *counts.entry(key).or_insert(0) += 1;
    }

    let mut duplicates: Vec<String> = counts
        .into_iter()
        .filter_map(|(key, count)| (count > 1).then_some(key))
        .collect();
    duplicates.sort();
    duplicates
}

fn duplicate_key(release: &ParsedReleaseTitle) -> String {
    format!(
        "{}|{}|{}",
        release
            .artist
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_lowercase(),
        release
            .album
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_lowercase(),
        release.quality_key(),
    )
}

fn score_release_with_words(
    release: &ParsedReleaseTitle,
    options: &ReleaseFilterOptions,
    normalized_preferred_words: &HashSet<String>,
    normalized_custom_rules: &[NormalizedCustomFormatRule],
) -> i32 {
    let quality_score = match release.quality {
        AudioQuality::Flac | AudioQuality::Alac => 200,
        AudioQuality::Mp3 => 120,
        AudioQuality::Aac => 100,
        AudioQuality::Unknown => 20,
    } as i64;

    let bitrate_score = release
        .bitrate_kbps
        .map(|value| (value / 10) as i64)
        .unwrap_or(0);

    let group_score = release
        .release_group
        .as_ref()
        .and_then(|group| {
            options
                .preferred_release_groups
                .iter()
                .any(|preferred| preferred.eq_ignore_ascii_case(group))
                .then_some(75)
        })
        .unwrap_or(0) as i64;

    let normalized_title =
        if normalized_preferred_words.is_empty() && normalized_custom_rules.is_empty() {
            None
        } else {
            Some(normalize_whitespace(&release.original_title).to_lowercase())
        };

    let preferred_word_score = normalized_title.as_deref().map_or(0, |title| {
        (preferred_word_matches(release, title, normalized_preferred_words) as i64) * 30
    });

    let custom_format_score = normalized_title.as_deref().map_or(0, |title| {
        custom_format_bonus(title, normalized_custom_rules)
    });

    (quality_score + bitrate_score + group_score + preferred_word_score + custom_format_score)
        .clamp(SCORE_MIN, SCORE_MAX) as i32
}

fn custom_format_bonus(
    normalized_title: &str,
    normalized_custom_rules: &[NormalizedCustomFormatRule],
) -> i64 {
    if normalized_custom_rules.is_empty() {
        return 0;
    }

    normalized_custom_rules
        .iter()
        .filter(|rule| {
            rule.keywords
                .iter()
                .any(|keyword| normalized_title.contains(keyword.as_str()))
        })
        .map(|rule| rule.score_bonus)
        .sum()
}

fn preferred_word_matches(
    release: &ParsedReleaseTitle,
    normalized_title: &str,
    normalized_preferred_words: &HashSet<String>,
) -> usize {
    if normalized_preferred_words.is_empty() {
        return 0;
    }

    let artist = release.artist.as_ref().map(|value| value.to_lowercase());
    let album = release.album.as_ref().map(|value| value.to_lowercase());
    let group = release
        .release_group
        .as_ref()
        .map(|value| value.to_lowercase());

    normalized_preferred_words
        .iter()
        .filter(|word| {
            let word = word.as_str();
            normalized_title.contains(word)
                || artist.as_ref().is_some_and(|value| value.contains(word))
                || album.as_ref().is_some_and(|value| value.contains(word))
                || group.as_ref().is_some_and(|value| value.contains(word))
        })
        .count()
}

fn normalize_preferred_words(preferred_words: &[String]) -> HashSet<String> {
    preferred_words
        .iter()
        .map(|word| normalize_whitespace(word).to_lowercase())
        .filter(|word| !word.is_empty())
        .collect()
}

fn normalize_custom_format_rules(rules: &[CustomFormatRule]) -> Vec<NormalizedCustomFormatRule> {
    rules
        .iter()
        .filter_map(|rule| {
            let keywords = rule
                .keywords
                .iter()
                .map(|word| normalize_whitespace(word).to_lowercase())
                .filter(|word| !word.is_empty())
                .collect::<Vec<_>>();

            if keywords.is_empty() {
                return None;
            }

            Some(NormalizedCustomFormatRule {
                keywords,
                score_bonus: i64::from(rule.score_bonus),
            })
        })
        .collect()
}

fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn detect_quality(title: &str) -> AudioQuality {
    let lowercase = title.to_lowercase();

    if lowercase.contains("flac") {
        AudioQuality::Flac
    } else if lowercase.contains("alac") {
        AudioQuality::Alac
    } else if lowercase.contains("mp3") || lowercase.contains("v0") || lowercase.contains("v2") {
        AudioQuality::Mp3
    } else if lowercase.contains("aac") || lowercase.contains("m4a") {
        AudioQuality::Aac
    } else {
        AudioQuality::Unknown
    }
}

fn detect_bitrate_kbps(title: &str, quality: &AudioQuality) -> Option<u32> {
    lazy_static! {
        static ref BITRATE_REGEX: Regex =
            Regex::new(r"(?i)\b(?P<bitrate>\d{2,4})\s?(?:kbps|k)\b").expect("valid bitrate regex");
    }

    if let Some(captures) = BITRATE_REGEX.captures(title) {
        if let Some(value) = captures.name("bitrate") {
            if let Ok(parsed) = value.as_str().parse::<u32>() {
                return Some(parsed);
            }
        }
    }

    let lowercase = title.to_lowercase();
    if lowercase.contains("v0") {
        return Some(245);
    }
    if lowercase.contains("v2") {
        return Some(190);
    }

    match quality {
        AudioQuality::Flac | AudioQuality::Alac => None,
        _ => None,
    }
}

fn detect_release_group(title: &str) -> Option<String> {
    lazy_static! {
        static ref GROUP_REGEX: Regex =
            Regex::new(r"-(?P<group>[A-Za-z0-9][A-Za-z0-9_.-]{1,31})$").expect("valid group regex");
    }

    GROUP_REGEX
        .captures(title)
        .and_then(|captures| captures.name("group").map(|m| m.as_str().to_string()))
}

fn extract_artist_album(title: &str) -> (Option<String>, Option<String>) {
    let stripped = strip_bracketed_chunks(title);
    let stripped = strip_release_group_suffix(&stripped);

    let Some((artist_raw, album_raw)) = stripped.split_once(" - ") else {
        return (None, None);
    };

    let artist = clean_component(artist_raw);
    let album = clean_component(&strip_quality_bitrate_tokens(album_raw));

    (artist, album)
}

fn strip_quality_bitrate_tokens(value: &str) -> String {
    lazy_static! {
        static ref QUALITY_TOKEN_REGEX: Regex =
            Regex::new(r"(?i)\b(flac|alac|mp3|aac|m4a|v0|v2)\b|\b\d{2,4}\s?(?:kbps|k)\b")
                .expect("valid quality token regex");
    }

    normalize_whitespace(QUALITY_TOKEN_REGEX.replace_all(value, "").trim())
}

fn strip_bracketed_chunks(value: &str) -> String {
    lazy_static! {
        static ref BRACKETED_REGEX: Regex =
            Regex::new(r"\[[^\]]*\]|\([^\)]*\)").expect("valid bracketed regex");
    }

    BRACKETED_REGEX.replace_all(value, "").to_string()
}

fn strip_release_group_suffix(value: &str) -> String {
    lazy_static! {
        static ref GROUP_SUFFIX_REGEX: Regex =
            Regex::new(r"\s*-[A-Za-z0-9][A-Za-z0-9_.-]{1,31}$").expect("valid group suffix regex");
    }

    GROUP_SUFFIX_REGEX.replace(value, "").to_string()
}

fn clean_component(value: &str) -> Option<String> {
    let cleaned = value.trim().trim_matches('-').trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

impl AudioQuality {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioQuality::Flac => "flac",
            AudioQuality::Mp3 => "mp3",
            AudioQuality::Aac => "aac",
            AudioQuality::Alac => "alac",
            AudioQuality::Unknown => "unknown",
        }
    }
}

impl ParsedReleaseTitle {
    fn quality_key(&self) -> &'static str {
        self.quality.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        deduplicate_releases, filter_releases, find_duplicate_keys, parse_release_title,
        rank_releases, AudioQuality, CustomFormatRule, ParsedReleaseTitle, ReleaseFilterOptions,
    };

    #[test]
    fn parses_artist_album_quality_and_group() {
        let parsed = parse_release_title("Daft Punk - Random Access Memories [FLAC]-RLSGRP");

        assert_eq!(parsed.artist.as_deref(), Some("Daft Punk"));
        assert_eq!(parsed.album.as_deref(), Some("Random Access Memories"));
        assert_eq!(parsed.quality, AudioQuality::Flac);
        assert_eq!(parsed.bitrate_kbps, None);
        assert_eq!(parsed.release_group.as_deref(), Some("RLSGRP"));
    }

    #[test]
    fn parses_bitrate_from_mp3_title() {
        let parsed = parse_release_title("Nirvana - Nevermind 320kbps MP3-GroupX");

        assert_eq!(parsed.artist.as_deref(), Some("Nirvana"));
        assert_eq!(parsed.album.as_deref(), Some("Nevermind"));
        assert_eq!(parsed.quality, AudioQuality::Mp3);
        assert_eq!(parsed.bitrate_kbps, Some(320));
        assert_eq!(parsed.release_group.as_deref(), Some("GroupX"));
    }

    #[test]
    fn filters_by_quality_and_bitrate() {
        let releases = vec![
            parse_release_title("Artist - Album [FLAC]-AAA"),
            parse_release_title("Artist - Album 192kbps MP3-BBB"),
            parse_release_title("Artist - Album 320kbps MP3-CCC"),
        ];

        let options = ReleaseFilterOptions {
            preferred_qualities: vec![AudioQuality::Mp3],
            min_bitrate_kbps: Some(256),
            preferred_release_groups: vec![],
            preferred_words: vec![],
            custom_format_rules: vec![],
        };

        let filtered = filter_releases(&releases, &options);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].bitrate_kbps, Some(320));
    }

    #[test]
    fn lossless_always_passes_min_bitrate_filter() {
        let releases = vec![
            parse_release_title("Artist - Album [FLAC]-AAA"),
            parse_release_title("Artist - Album [ALAC]-BBB"),
            parse_release_title("Artist - Album 128kbps MP3-CCC"),
        ];

        let options = ReleaseFilterOptions {
            preferred_qualities: vec![],
            min_bitrate_kbps: Some(256),
            preferred_release_groups: vec![],
            preferred_words: vec![],
            custom_format_rules: vec![],
        };

        let filtered = filter_releases(&releases, &options);
        assert_eq!(filtered.len(), 2);
        assert!(filtered
            .iter()
            .all(|r| matches!(r.quality, AudioQuality::Flac | AudioQuality::Alac)));
    }

    #[test]
    fn original_title_stores_raw_input() {
        let raw = "  Daft Punk  -  Discovery  [FLAC]-GRP  ";
        let parsed = parse_release_title(raw);
        assert_eq!(parsed.original_title, raw);
    }

    #[test]
    fn ranks_preferred_group_higher_when_quality_same() {
        let releases = vec![
            parse_release_title("Artist - Album 320kbps MP3-Preferred"),
            parse_release_title("Artist - Album 320kbps MP3-Other"),
        ];

        let options = ReleaseFilterOptions {
            preferred_qualities: vec![],
            min_bitrate_kbps: None,
            preferred_release_groups: vec!["Preferred".to_string()],
            preferred_words: vec![],
            custom_format_rules: vec![],
        };

        let ranked = rank_releases(releases, &options);
        assert_eq!(ranked[0].release_group.as_deref(), Some("Preferred"));
    }

    #[test]
    fn ranks_lossless_above_lossy() {
        let releases = vec![
            ParsedReleaseTitle {
                original_title: "A".to_string(),
                artist: Some("Artist".to_string()),
                album: Some("Album".to_string()),
                quality: AudioQuality::Mp3,
                bitrate_kbps: Some(320),
                release_group: Some("Group1".to_string()),
            },
            ParsedReleaseTitle {
                original_title: "B".to_string(),
                artist: Some("Artist".to_string()),
                album: Some("Album".to_string()),
                quality: AudioQuality::Flac,
                bitrate_kbps: None,
                release_group: Some("Group2".to_string()),
            },
        ];

        let ranked = rank_releases(releases, &ReleaseFilterOptions::default());
        assert_eq!(ranked[0].quality, AudioQuality::Flac);
    }

    #[test]
    fn ranks_preferred_word_higher_when_quality_same() {
        let releases = vec![
            parse_release_title("Artist - Album Deluxe Edition 320kbps MP3-GroupA"),
            parse_release_title("Artist - Album Standard Edition 320kbps MP3-GroupB"),
        ];

        let options = ReleaseFilterOptions {
            preferred_qualities: vec![],
            min_bitrate_kbps: None,
            preferred_release_groups: vec![],
            preferred_words: vec!["DELUXE".to_string()],
            custom_format_rules: vec![],
        };

        let ranked = rank_releases(releases, &options);
        assert!(ranked[0].original_title.to_lowercase().contains("deluxe"));
    }

    #[test]
    fn preferred_word_can_match_release_group() {
        let releases = vec![
            parse_release_title("Artist - Album 320kbps MP3-ScenePrime"),
            parse_release_title("Artist - Album 320kbps MP3-OtherGroup"),
        ];

        let options = ReleaseFilterOptions {
            preferred_qualities: vec![],
            min_bitrate_kbps: None,
            preferred_release_groups: vec![],
            preferred_words: vec!["sceneprime".to_string()],
            custom_format_rules: vec![],
        };

        let ranked = rank_releases(releases, &options);
        assert_eq!(ranked[0].release_group.as_deref(), Some("ScenePrime"));
    }

    #[test]
    fn preferred_phrase_matches_original_title_with_irregular_whitespace() {
        let releases = vec![
            parse_release_title("Daft    Punk    Live Set FLAC"),
            parse_release_title("Another Artist Live Set FLAC"),
        ];

        let options = ReleaseFilterOptions {
            preferred_qualities: vec![],
            min_bitrate_kbps: None,
            preferred_release_groups: vec![],
            preferred_words: vec!["daft punk".to_string()],
            custom_format_rules: vec![],
        };

        let ranked = rank_releases(releases, &options);
        assert!(ranked[0].original_title.contains("Daft"));
    }

    #[test]
    fn duplicate_key_detection_finds_matching_artist_album_quality() {
        let releases = vec![
            parse_release_title("Artist - Album 320kbps MP3-GRP1"),
            parse_release_title("Artist - Album 192kbps MP3-GRP2"),
            parse_release_title("Artist - Album [FLAC]-GRP3"),
        ];

        let keys = find_duplicate_keys(&releases);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "artist|album|mp3");
    }

    #[test]
    fn deduplicate_keeps_best_scored_release_per_key() {
        let releases = vec![
            parse_release_title("Artist - Album 192kbps MP3-GRP1"),
            parse_release_title("Artist - Album 320kbps MP3-GRP2"),
            parse_release_title("Artist - Album [FLAC]-GRP3"),
            parse_release_title("Artist - Album [FLAC]-GRP4"),
        ];

        let deduped = deduplicate_releases(&releases);

        assert_eq!(deduped.len(), 2);
        assert!(deduped.iter().any(
            |release| release.quality == AudioQuality::Mp3 && release.bitrate_kbps == Some(320)
        ));
        assert!(
            deduped
                .iter()
                .filter(|release| release.quality == AudioQuality::Flac)
                .count()
                == 1
        );
    }

    #[test]
    fn ranks_custom_format_rule_higher_when_quality_same() {
        let releases = vec![
            parse_release_title("Artist - Album 320kbps MP3 [MQA]-GroupA"),
            parse_release_title("Artist - Album 320kbps MP3-GroupB"),
        ];

        let options = ReleaseFilterOptions {
            preferred_qualities: vec![],
            min_bitrate_kbps: None,
            preferred_release_groups: vec![],
            preferred_words: vec![],
            custom_format_rules: vec![CustomFormatRule {
                name: "MQA".to_string(),
                keywords: vec!["mqa".to_string()],
                score_bonus: 60,
            }],
        };

        let ranked = rank_releases(releases, &options);
        assert!(ranked[0].original_title.to_lowercase().contains("mqa"));
    }

    #[test]
    fn custom_format_keyword_matches_with_irregular_whitespace() {
        let releases = vec![
            parse_release_title("Artist - Album MQA Deluxe Edition 320kbps MP3-GroupA"),
            parse_release_title("Artist - Album Standard Edition 320kbps MP3-GroupB"),
        ];

        let options = ReleaseFilterOptions {
            preferred_qualities: vec![],
            min_bitrate_kbps: None,
            preferred_release_groups: vec![],
            preferred_words: vec![],
            custom_format_rules: vec![CustomFormatRule {
                name: "MQA Deluxe".to_string(),
                keywords: vec!["mqa   deluxe".to_string()],
                score_bonus: 80,
            }],
        };

        let ranked = rank_releases(releases, &options);
        assert!(ranked[0]
            .original_title
            .to_lowercase()
            .contains("mqa deluxe"));
    }

    #[test]
    fn custom_format_score_uses_saturating_total() {
        let releases = vec![
            parse_release_title("Artist - Album MQA 320kbps MP3-GroupA"),
            parse_release_title("Artist - Album 320kbps MP3-GroupB"),
        ];

        let options = ReleaseFilterOptions {
            preferred_qualities: vec![],
            min_bitrate_kbps: None,
            preferred_release_groups: vec![],
            preferred_words: vec![],
            custom_format_rules: vec![
                CustomFormatRule {
                    name: "Rule 1".to_string(),
                    keywords: vec!["mqa".to_string()],
                    score_bonus: i32::MAX,
                },
                CustomFormatRule {
                    name: "Rule 2".to_string(),
                    keywords: vec!["mqa".to_string()],
                    score_bonus: i32::MAX,
                },
            ],
        };

        let ranked = rank_releases(releases, &options);
        assert!(ranked[0].original_title.to_lowercase().contains("mqa"));
    }
}
