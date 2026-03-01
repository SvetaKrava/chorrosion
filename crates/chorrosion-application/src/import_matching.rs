// SPDX-License-Identifier: GPL-3.0-or-later

use crate::filename_heuristics::FilenameHeuristicsService;
use chorrosion_domain::{AlbumId, ArtistId};
use lazy_static::lazy_static;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::warn;

lazy_static! {
    static ref BITRATE_REGEX: Regex =
        Regex::new(r"(?i)\b(?P<bitrate>\d{3})\s?kbps\b").expect("bitrate regex is valid");
}

#[derive(Debug, Error)]
pub enum ImportMatchingError {
    #[error("path does not exist: {0}")]
    PathNotFound(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("failed to parse metadata: {0}")]
    MetadataParsing(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedAudioFile {
    pub path: PathBuf,
    pub extension: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataSource {
    EmbeddedTags,
    FilenameHeuristics,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawTrackMetadata {
    pub file_path: PathBuf,
    pub embedded_artist: Option<String>,
    pub embedded_album: Option<String>,
    pub embedded_title: Option<String>,
    pub duration_seconds: Option<u32>,
    pub bitrate_kbps: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTrackMetadata {
    pub file_path: PathBuf,
    pub artist: String,
    pub album: String,
    pub title: String,
    pub duration_seconds: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub source: MetadataSource,
}

#[derive(Debug, Clone)]
pub struct CatalogAlbum {
    pub artist_id: ArtistId,
    pub album_id: AlbumId,
    pub artist_name: String,
    pub album_title: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchStrategy {
    Exact,
    Fuzzy,
}

#[derive(Debug, Clone)]
pub struct CatalogAlbumMatch {
    pub artist_id: ArtistId,
    pub album_id: AlbumId,
    pub confidence: f32,
    pub strategy: MatchStrategy,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportDecision {
    Import {
        artist_id: ArtistId,
        album_id: AlbumId,
        confidence: f32,
    },
    NeedsReview {
        reason: String,
        confidence: f32,
    },
    Skip {
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct ImportEvaluation {
    pub best_match: Option<CatalogAlbumMatch>,
    pub decision: ImportDecision,
}

pub fn scan_audio_files(root: impl AsRef<Path>) -> Result<Vec<ScannedAudioFile>, ImportMatchingError> {
    let root = root.as_ref();
    if !root.exists() {
        return Err(ImportMatchingError::PathNotFound(root.display().to_string()));
    }

    let mut scanned = Vec::new();
    visit_directory(root, &mut scanned)?;
    scanned.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(scanned)
}

pub fn parse_track_metadata(raw: &RawTrackMetadata) -> Result<ParsedTrackMetadata, ImportMatchingError> {
    if !raw.file_path.exists() {
        return Err(ImportMatchingError::PathNotFound(
            raw.file_path.display().to_string(),
        ));
    }

    let embedded_artist = normalize_optional(raw.embedded_artist.as_deref());
    let embedded_album = normalize_optional(raw.embedded_album.as_deref());
    let embedded_title = normalize_optional(raw.embedded_title.as_deref());

    if let (Some(artist), Some(album), Some(title)) = (embedded_artist, embedded_album, embedded_title) {
        return Ok(ParsedTrackMetadata {
            file_path: raw.file_path.clone(),
            artist,
            album,
            title,
            duration_seconds: raw.duration_seconds,
            bitrate_kbps: raw
                .bitrate_kbps
                .or_else(|| extract_bitrate_from_filename(&raw.file_path)),
            source: MetadataSource::EmbeddedTags,
        });
    }

    let folder_album = raw
        .file_path
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|segment| segment.to_str())
        .map(str::to_owned);
    let folder_artist = raw
        .file_path
        .parent()
        .and_then(Path::parent)
        .and_then(|path| path.file_name())
        .and_then(|segment| segment.to_str())
        .map(str::to_owned);

    let parser = FilenameHeuristicsService;
    let parsed = parser
        .parse_filename(
            &raw.file_path,
            folder_artist.as_deref(),
            folder_album.as_deref(),
        )
        .map_err(|err| ImportMatchingError::MetadataParsing(err.to_string()))?;

    let artist = parsed
        .artist
        .and_then(|value| normalize_optional(Some(&value)))
        .ok_or_else(|| ImportMatchingError::MetadataParsing("artist missing from metadata".to_string()))?;
    let album = parsed
        .album
        .and_then(|value| normalize_optional(Some(&value)))
        .ok_or_else(|| ImportMatchingError::MetadataParsing("album missing from metadata".to_string()))?;
    let title = parsed
        .title
        .and_then(|value| normalize_optional(Some(&value)))
        .ok_or_else(|| ImportMatchingError::MetadataParsing("title missing from metadata".to_string()))?;

    Ok(ParsedTrackMetadata {
        file_path: raw.file_path.clone(),
        artist,
        album,
        title,
        duration_seconds: raw.duration_seconds,
        bitrate_kbps: raw
            .bitrate_kbps
            .or_else(|| extract_bitrate_from_filename(&raw.file_path)),
        source: MetadataSource::FilenameHeuristics,
    })
}

pub fn evaluate_import_match(
    metadata: &ParsedTrackMetadata,
    catalog: &[CatalogAlbum],
    fuzzy_threshold: f32,
    auto_import_threshold: f32,
) -> ImportEvaluation {
    let fuzzy_threshold = clamp_threshold("fuzzy_threshold", fuzzy_threshold, 0.0);
    let auto_import_threshold = clamp_threshold("auto_import_threshold", auto_import_threshold, 1.0);

    if catalog.is_empty() {
        return ImportEvaluation {
            best_match: None,
            decision: ImportDecision::Skip {
                reason: "catalog is empty".to_string(),
            },
        };
    }

    let best_match = find_best_catalog_match(metadata, catalog, fuzzy_threshold);
    let decision = match &best_match {
        Some(candidate) if candidate.confidence >= auto_import_threshold => ImportDecision::Import {
            artist_id: candidate.artist_id,
            album_id: candidate.album_id,
            confidence: candidate.confidence,
        },
        Some(candidate) => ImportDecision::NeedsReview {
            reason: "match confidence below auto-import threshold".to_string(),
            confidence: candidate.confidence,
        },
        None => ImportDecision::Skip {
            reason: "no matching artist/album candidate found".to_string(),
        },
    };

    ImportEvaluation { best_match, decision }
}

fn clamp_threshold(name: &str, value: f32, non_finite_default: f32) -> f32 {
    if !value.is_finite() {
        warn!(target: "application", name, value, "threshold is not finite, using default {non_finite_default}");
        return non_finite_default;
    }
    if !(0.0..=1.0).contains(&value) {
        let clamped = value.clamp(0.0, 1.0);
        warn!(target: "application", name, value, clamped, "threshold out of [0.0, 1.0] range, clamping");
        return clamped;
    }
    value
}

fn find_best_catalog_match(
    metadata: &ParsedTrackMetadata,
    catalog: &[CatalogAlbum],
    fuzzy_threshold: f32,
) -> Option<CatalogAlbumMatch> {
    catalog
        .iter()
        .map(|candidate| {
            let artist_similarity = normalized_similarity(&metadata.artist, &candidate.artist_name);
            let album_similarity = normalized_similarity(&metadata.album, &candidate.album_title);
            let confidence = ((artist_similarity * 0.6) + (album_similarity * 0.4)).clamp(0.0, 1.0);
            let strategy = if artist_similarity == 1.0 && album_similarity == 1.0 {
                MatchStrategy::Exact
            } else {
                MatchStrategy::Fuzzy
            };

            (candidate, confidence, strategy)
        })
        .filter(|(_, confidence, strategy)| {
            if matches!(strategy, MatchStrategy::Exact) {
                true
            } else {
                *confidence >= fuzzy_threshold
            }
        })
        .max_by(|left, right| left.1.partial_cmp(&right.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(candidate, confidence, strategy)| CatalogAlbumMatch {
            artist_id: candidate.artist_id,
            album_id: candidate.album_id,
            confidence,
            strategy,
        })
}

fn visit_directory(
    directory: &Path,
    scanned: &mut Vec<ScannedAudioFile>,
) -> Result<(), ImportMatchingError> {
    let entries = fs::read_dir(directory).map_err(|err| ImportMatchingError::Io(err.to_string()))?;

    for entry in entries {
        let entry = entry.map_err(|err| ImportMatchingError::Io(err.to_string()))?;
        let path = entry.path();

        let file_type = entry
            .file_type()
            .map_err(|err| ImportMatchingError::Io(err.to_string()))?;

        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            visit_directory(&path, scanned)?;
            continue;
        }

        let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };

        let normalized_extension = extension.to_ascii_lowercase();
        if !is_audio_extension(&normalized_extension) {
            continue;
        }

        let metadata = fs::metadata(&path).map_err(|err| ImportMatchingError::Io(err.to_string()))?;
        scanned.push(ScannedAudioFile {
            path,
            extension: normalized_extension,
            size_bytes: metadata.len(),
        });
    }

    Ok(())
}

fn is_audio_extension(extension: &str) -> bool {
    matches!(
        extension,
        "mp3" | "flac" | "m4a" | "aac" | "ogg" | "opus" | "wav" | "wv" | "ape" | "dsf"
    )
}

fn extract_bitrate_from_filename(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_str()?;
    BITRATE_REGEX
        .captures(stem)
        .and_then(|captures| captures.name("bitrate"))
        .and_then(|value| value.as_str().parse::<u32>().ok())
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn normalized_similarity(left: &str, right: &str) -> f32 {
    let left = normalize_for_match(left);
    let right = normalize_for_match(right);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    if left == right {
        return 1.0;
    }

    let distance = levenshtein_distance(&left, &right) as f32;
    let max_len = left.chars().count().max(right.chars().count()) as f32;
    (1.0 - (distance / max_len)).clamp(0.0, 1.0)
}

fn normalize_for_match(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn levenshtein_distance(left: &str, right: &str) -> usize {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();

    if left_chars.is_empty() {
        return right_chars.len();
    }
    if right_chars.is_empty() {
        return left_chars.len();
    }

    let mut previous_row: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current_row: Vec<usize> = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left_chars.iter().enumerate() {
        current_row[0] = left_index + 1;
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let insert_cost = current_row[right_index] + 1;
            let delete_cost = previous_row[right_index + 1] + 1;
            let replace_cost = previous_row[right_index] + usize::from(left_char != right_char);
            current_row[right_index + 1] = insert_cost.min(delete_cost).min(replace_cost);
        }
        std::mem::swap(&mut previous_row, &mut current_row);
    }

    previous_row[right_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_audio_files_recursively_filters_supported_extensions() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let album_dir = root.path().join("artist").join("album");
        fs::create_dir_all(&album_dir).expect("nested dir should be created");

        let audio = album_dir.join("01 - Track.mp3");
        let image = album_dir.join("cover.jpg");
        fs::write(&audio, b"audio-data").expect("audio file should exist");
        fs::write(&image, b"image-data").expect("image file should exist");

        let scanned = scan_audio_files(root.path()).expect("scan should succeed");

        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].path, audio);
        assert_eq!(scanned[0].extension, "mp3");
    }

    #[test]
    fn parse_track_metadata_prefers_embedded_tags() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let file = root.path().join("any.mp3");
        fs::write(&file, b"audio-data").expect("file should exist");

        let parsed = parse_track_metadata(&RawTrackMetadata {
            file_path: file.clone(),
            embedded_artist: Some("Autechre".to_string()),
            embedded_album: Some("Amber".to_string()),
            embedded_title: Some("Foil".to_string()),
            duration_seconds: Some(321),
            bitrate_kbps: Some(320),
        })
        .expect("metadata parsing should succeed");

        assert_eq!(parsed.artist, "Autechre");
        assert_eq!(parsed.album, "Amber");
        assert_eq!(parsed.title, "Foil");
        assert_eq!(parsed.source, MetadataSource::EmbeddedTags);
    }

    #[test]
    fn parse_track_metadata_falls_back_to_filename_heuristics() {
        let root = tempfile::tempdir().expect("temp dir should be created");
        let album_dir = root
            .path()
            .join("Boards of Canada")
            .join("Music Has the Right to Children");
        fs::create_dir_all(&album_dir).expect("nested dir should exist");

        let file = album_dir.join("Boards of Canada - 01 - Wildlife Analysis 320kbps.mp3");
        fs::write(&file, b"audio-data").expect("file should exist");

        let parsed = parse_track_metadata(&RawTrackMetadata {
            file_path: file.clone(),
            embedded_artist: None,
            embedded_album: None,
            embedded_title: None,
            duration_seconds: None,
            bitrate_kbps: None,
        })
        .expect("fallback parsing should succeed");

        assert_eq!(parsed.artist, "Boards of Canada");
        assert_eq!(parsed.album, "Music Has the Right to Children");
        assert_eq!(parsed.source, MetadataSource::FilenameHeuristics);
        assert_eq!(parsed.bitrate_kbps, Some(320));
    }

    #[test]
    fn evaluate_import_match_supports_fuzzy_matching() {
        let metadata = ParsedTrackMetadata {
            file_path: PathBuf::from("test.mp3"),
            artist: "Boards of Canda".to_string(),
            album: "Music Has The Right To Children".to_string(),
            title: "Roygbiv".to_string(),
            duration_seconds: None,
            bitrate_kbps: None,
            source: MetadataSource::FilenameHeuristics,
        };

        let artist_id = ArtistId::new();
        let album_id = AlbumId::new();
        let catalog = vec![CatalogAlbum {
            artist_id,
            album_id,
            artist_name: "Boards of Canada".to_string(),
            album_title: "Music Has the Right to Children".to_string(),
        }];

        let result = evaluate_import_match(&metadata, &catalog, 0.70, 0.80);
        assert!(result.best_match.is_some());
        assert!(matches!(
            result.decision,
            ImportDecision::Import {
                artist_id: matched_artist,
                album_id: matched_album,
                confidence: _
            } if matched_artist == artist_id && matched_album == album_id
        ));
    }

    #[test]
    fn evaluate_import_match_requires_review_below_threshold() {
        let metadata = ParsedTrackMetadata {
            file_path: PathBuf::from("test.mp3"),
            artist: "Unknown Artist".to_string(),
            album: "Unknown Album".to_string(),
            title: "Unknown Track".to_string(),
            duration_seconds: None,
            bitrate_kbps: None,
            source: MetadataSource::FilenameHeuristics,
        };

        let catalog = vec![CatalogAlbum {
            artist_id: ArtistId::new(),
            album_id: AlbumId::new(),
            artist_name: "Known Artist".to_string(),
            album_title: "Known Album".to_string(),
        }];

        let result = evaluate_import_match(&metadata, &catalog, 0.10, 0.95);
        assert!(matches!(result.decision, ImportDecision::NeedsReview { .. }));
    }
}