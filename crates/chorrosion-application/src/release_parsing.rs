// SPDX-License-Identifier: GPL-3.0-or-later
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

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
}

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
    releases.sort_by_key(|release| std::cmp::Reverse(score_release(release, options)));
    releases
}

fn score_release(release: &ParsedReleaseTitle, options: &ReleaseFilterOptions) -> i32 {
    let quality_score = match release.quality {
        AudioQuality::Flac | AudioQuality::Alac => 200,
        AudioQuality::Mp3 => 120,
        AudioQuality::Aac => 100,
        AudioQuality::Unknown => 20,
    };

    let bitrate_score = release.bitrate_kbps.map(|value| (value / 10) as i32).unwrap_or(0);

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
        .unwrap_or(0);

    quality_score + bitrate_score + group_score
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
        static ref QUALITY_TOKEN_REGEX: Regex = Regex::new(
            r"(?i)\b(flac|alac|mp3|aac|m4a|v0|v2)\b|\b\d{2,4}\s?(?:kbps|k)\b"
        )
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

#[cfg(test)]
mod tests {
    use super::{
        filter_releases, parse_release_title, rank_releases, AudioQuality, ParsedReleaseTitle,
        ReleaseFilterOptions,
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
        };

        let filtered = filter_releases(&releases, &options);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|r| matches!(
            r.quality,
            AudioQuality::Flac | AudioQuality::Alac
        )));
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
}
