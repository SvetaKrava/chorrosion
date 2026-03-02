// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagFormat {
    Id3v2,
    VorbisComments,
    Mp4,
    Ape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkData {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagEmbeddingPayload {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub fingerprint_hash: Option<String>,
    pub artwork: Option<ArtworkData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRoundtripSnapshot {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub fingerprint_hash: Option<String>,
    pub has_artwork: bool,
}

#[derive(Debug, Clone)]
pub struct TagEmbeddingRequest {
    pub file_path: PathBuf,
    pub payload: TagEmbeddingPayload,
    pub quality_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TagEmbeddingOptions {
    pub enabled: bool,
    pub read_only: bool,
    pub verify_roundtrip: bool,
    pub overwrite_existing: bool,
    pub allowed_quality_names: Option<Vec<String>>,
}

impl Default for TagEmbeddingOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            read_only: false,
            verify_roundtrip: true,
            overwrite_existing: true,
            allowed_quality_names: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagEmbeddingOutcome {
    Embedded {
        format: TagFormat,
    },
    Skipped {
        reason: String,
    },
}

#[derive(Debug, Error)]
pub enum TagEmbeddingError {
    #[error("file not found: {0}")]
    FileNotFound(String),
    #[error("unsupported audio format: {0}")]
    UnsupportedFormat(String),
    #[error("tag backend failed: {0}")]
    Backend(String),
    #[error("roundtrip verification failed: {0}")]
    RoundtripVerification(String),
    #[error("file operation failed: {0}")]
    FileOperation(String),
}

pub trait TagEmbeddingBackend: Send + Sync {
    fn write_to_path(
        &self,
        file_path: &Path,
        format: TagFormat,
        payload: &TagEmbeddingPayload,
        overwrite_existing: bool,
    ) -> Result<(), String>;

    fn read_snapshot(
        &self,
        file_path: &Path,
        format: TagFormat,
    ) -> Result<TagRoundtripSnapshot, String>;
}

pub struct TagEmbeddingService {
    backend: Arc<dyn TagEmbeddingBackend>,
}

impl TagEmbeddingService {
    pub fn new(backend: Arc<dyn TagEmbeddingBackend>) -> Self {
        Self { backend }
    }

    pub fn embed_tags(
        &self,
        request: &TagEmbeddingRequest,
        options: &TagEmbeddingOptions,
    ) -> Result<TagEmbeddingOutcome, TagEmbeddingError> {
        if !options.enabled {
            return Ok(TagEmbeddingOutcome::Skipped {
                reason: "tag embedding disabled".to_string(),
            });
        }
        if options.read_only {
            return Ok(TagEmbeddingOutcome::Skipped {
                reason: "read-only mode enabled".to_string(),
            });
        }
        if !quality_allowed(request.quality_name.as_deref(), options.allowed_quality_names.as_ref())
        {
            return Ok(TagEmbeddingOutcome::Skipped {
                reason: "quality profile is not configured for tag embedding".to_string(),
            });
        }

        if !request.file_path.exists() {
            return Err(TagEmbeddingError::FileNotFound(
                request.file_path.display().to_string(),
            ));
        }

        let format = detect_tag_format(&request.file_path)?;
        let backup_path = backup_path_for(&request.file_path);
        let temp_path = temp_path_for(&request.file_path);

        fs::copy(&request.file_path, &backup_path)
            .map_err(|err| TagEmbeddingError::FileOperation(err.to_string()))?;
        fs::copy(&request.file_path, &temp_path)
            .map_err(|err| TagEmbeddingError::FileOperation(err.to_string()))?;

        if let Err(error) = self.backend.write_to_path(
            &temp_path,
            format,
            &request.payload,
            options.overwrite_existing,
        ) {
            restore_backup(&backup_path, &request.file_path)?;
            let _ = fs::remove_file(&temp_path);
            let _ = fs::remove_file(&backup_path);
            return Err(TagEmbeddingError::Backend(error));
        }

        if options.verify_roundtrip {
            let snapshot = match self.backend.read_snapshot(&temp_path, format) {
                Ok(snapshot) => snapshot,
                Err(err) => {
                    let _ = fs::remove_file(&temp_path);
                    let _ = fs::remove_file(&backup_path);
                    return Err(TagEmbeddingError::RoundtripVerification(err));
                }
            };

            if let Err(err) = verify_snapshot(&request.payload, &snapshot) {
                let _ = fs::remove_file(&temp_path);
                let _ = fs::remove_file(&backup_path);
                return Err(err);
            }
        }

        if let Err(error) = replace_file_atomically(&temp_path, &request.file_path) {
            restore_backup(&backup_path, &request.file_path)?;
            let _ = fs::remove_file(&temp_path);
            let _ = fs::remove_file(&backup_path);
            return Err(error);
        }

        let _ = fs::remove_file(&backup_path);

        Ok(TagEmbeddingOutcome::Embedded { format })
    }
}

fn detect_tag_format(path: &Path) -> Result<TagFormat, TagEmbeddingError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    match extension.as_deref() {
        Some("mp3") => Ok(TagFormat::Id3v2),
        Some("flac") | Some("ogg") | Some("opus") => Ok(TagFormat::VorbisComments),
        Some("m4a") | Some("aac") | Some("mp4") => Ok(TagFormat::Mp4),
        Some("wv") | Some("ape") => Ok(TagFormat::Ape),
        None => Err(TagEmbeddingError::UnsupportedFormat(format!(
            "missing or invalid file extension for path {}",
            path.display()
        ))),
        Some(other) => Err(TagEmbeddingError::UnsupportedFormat(other.to_string())),
    }
}

fn verify_snapshot(
    payload: &TagEmbeddingPayload,
    snapshot: &TagRoundtripSnapshot,
) -> Result<(), TagEmbeddingError> {
    if let Some(artist) = payload.artist.as_deref() {
        if snapshot.artist.as_deref() != Some(artist) {
            return Err(TagEmbeddingError::RoundtripVerification(
                "artist did not persist in roundtrip".to_string(),
            ));
        }
    }
    if let Some(album) = payload.album.as_deref() {
        if snapshot.album.as_deref() != Some(album) {
            return Err(TagEmbeddingError::RoundtripVerification(
                "album did not persist in roundtrip".to_string(),
            ));
        }
    }
    if let Some(title) = payload.title.as_deref() {
        if snapshot.title.as_deref() != Some(title) {
            return Err(TagEmbeddingError::RoundtripVerification(
                "title did not persist in roundtrip".to_string(),
            ));
        }
    }
    if let Some(fingerprint_hash) = payload.fingerprint_hash.as_deref() {
        if snapshot.fingerprint_hash.as_deref() != Some(fingerprint_hash) {
            return Err(TagEmbeddingError::RoundtripVerification(
                "fingerprint hash did not persist in roundtrip".to_string(),
            ));
        }
    }
    if payload.artwork.is_some() && !snapshot.has_artwork {
        return Err(TagEmbeddingError::RoundtripVerification(
            "artwork did not persist in roundtrip".to_string(),
        ));
    }
    Ok(())
}

fn quality_allowed(quality: Option<&str>, allowed: Option<&Vec<String>>) -> bool {
    let Some(allowed) = allowed else {
        return true;
    };

    let Some(quality) = quality else {
        return false;
    };

    allowed
        .iter()
        .any(|configured| configured.eq_ignore_ascii_case(quality))
}

fn backup_path_for(file_path: &Path) -> PathBuf {
    file_path.with_extension(format!(
        "{}.chorrosion.bak",
        file_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
    ))
}

fn temp_path_for(file_path: &Path) -> PathBuf {
    file_path.with_extension(format!(
        "{}.chorrosion.tmp",
        file_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
    ))
}

fn restore_backup(backup_path: &Path, destination: &Path) -> Result<(), TagEmbeddingError> {
    fs::copy(backup_path, destination)
        .map_err(|err| TagEmbeddingError::FileOperation(err.to_string()))?;
    Ok(())
}

#[cfg(unix)]
fn replace_file_atomically(temp_path: &Path, destination: &Path) -> Result<(), TagEmbeddingError> {
    // On Unix, std::fs::rename uses rename(2), which atomically replaces
    // the destination if it already exists.
    fs::rename(temp_path, destination)
        .map_err(|err| TagEmbeddingError::FileOperation(err.to_string()))
}

#[cfg(not(unix))]
fn replace_file_atomically(temp_path: &Path, destination: &Path) -> Result<(), TagEmbeddingError> {
    // Remove the destination so we can rename the temp into place.
    // The caller already holds a backup and is responsible for restoring it on error.
    if destination.exists() {
        fs::remove_file(destination)
            .map_err(|err| TagEmbeddingError::FileOperation(err.to_string()))?;
    }
    fs::rename(temp_path, destination)
        .map_err(|err| TagEmbeddingError::FileOperation(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct FakeTagBackend {
        fail_write: bool,
        fail_read: bool,
        roundtrip_snapshot: TagRoundtripSnapshot,
        writes: Mutex<HashMap<PathBuf, Vec<u8>>>,
    }

    impl FakeTagBackend {
        fn success() -> Self {
            Self {
                fail_write: false,
                fail_read: false,
                roundtrip_snapshot: TagRoundtripSnapshot {
                    artist: Some("Artist".to_string()),
                    album: Some("Album".to_string()),
                    title: Some("Track".to_string()),
                    fingerprint_hash: Some("abc123".to_string()),
                    has_artwork: true,
                },
                writes: Mutex::new(HashMap::new()),
            }
        }
    }

    impl TagEmbeddingBackend for FakeTagBackend {
        fn write_to_path(
            &self,
            file_path: &Path,
            _format: TagFormat,
            _payload: &TagEmbeddingPayload,
            _overwrite_existing: bool,
        ) -> Result<(), String> {
            if self.fail_write {
                return Err("forced write failure".to_string());
            }
            fs::write(file_path, b"embedded").map_err(|err| err.to_string())?;
            self.writes
                .lock()
                .expect("writes lock")
                .insert(file_path.to_path_buf(), b"embedded".to_vec());
            Ok(())
        }

        fn read_snapshot(
            &self,
            _file_path: &Path,
            _format: TagFormat,
        ) -> Result<TagRoundtripSnapshot, String> {
            if self.fail_read {
                return Err("forced roundtrip failure".to_string());
            }
            Ok(self.roundtrip_snapshot.clone())
        }
    }

    static TEMP_DIRS: std::sync::OnceLock<std::sync::Mutex<Vec<tempfile::TempDir>>> =
        std::sync::OnceLock::new();

    fn create_temp_file(extension: &str) -> PathBuf {
        let dir = tempfile::tempdir().expect("tempdir should be created");
        let file_path = dir.path().join(format!("song.{extension}"));
        fs::write(&file_path, b"original").expect("file should be written");
        // Store the TempDir so it is not dropped immediately; it will be
        // cleaned up automatically when the test process exits.
        let registry = TEMP_DIRS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
        registry
            .lock()
            .expect("TEMP_DIRS lock")
            .push(dir);
        file_path
    }

    fn request_for(path: PathBuf) -> TagEmbeddingRequest {
        TagEmbeddingRequest {
            file_path: path,
            payload: TagEmbeddingPayload {
                artist: Some("Artist".to_string()),
                album: Some("Album".to_string()),
                title: Some("Track".to_string()),
                track_number: Some(1),
                disc_number: Some(1),
                fingerprint_hash: Some("abc123".to_string()),
                artwork: Some(ArtworkData {
                    mime_type: "image/jpeg".to_string(),
                    bytes: vec![1, 2, 3],
                }),
            },
            quality_name: Some("Lossless".to_string()),
        }
    }

    #[test]
    fn skips_when_disabled() {
        let backend = Arc::new(FakeTagBackend::success());
        let service = TagEmbeddingService::new(backend);
        let request = request_for(create_temp_file("mp3"));
        let options = TagEmbeddingOptions {
            enabled: false,
            ..TagEmbeddingOptions::default()
        };

        let result = service.embed_tags(&request, &options).expect("should skip");
        assert!(matches!(result, TagEmbeddingOutcome::Skipped { .. }));
    }

    #[test]
    fn skips_when_quality_is_not_allowed() {
        let backend = Arc::new(FakeTagBackend::success());
        let service = TagEmbeddingService::new(backend);
        let request = request_for(create_temp_file("flac"));
        let options = TagEmbeddingOptions {
            allowed_quality_names: Some(vec!["Lossy".to_string()]),
            ..TagEmbeddingOptions::default()
        };

        let result = service.embed_tags(&request, &options).expect("should skip");
        assert!(matches!(result, TagEmbeddingOutcome::Skipped { .. }));
    }

    #[test]
    fn restores_original_when_backend_write_fails() {
        let backend = Arc::new(FakeTagBackend {
            fail_write: true,
            ..FakeTagBackend::success()
        });
        let service = TagEmbeddingService::new(backend);
        let file_path = create_temp_file("m4a");
        let request = request_for(file_path.clone());

        let result = service.embed_tags(&request, &TagEmbeddingOptions::default());
        assert!(matches!(result, Err(TagEmbeddingError::Backend(_))));

        let current = fs::read(&file_path).expect("file should still exist");
        assert_eq!(current, b"original");
    }

    #[test]
    fn embeds_tags_and_replaces_source_on_success() {
        let backend = Arc::new(FakeTagBackend::success());
        let service = TagEmbeddingService::new(backend);
        let file_path = create_temp_file("ape");
        let request = request_for(file_path.clone());

        let result = service
            .embed_tags(&request, &TagEmbeddingOptions::default())
            .expect("embedding should succeed");

        assert!(matches!(result, TagEmbeddingOutcome::Embedded { .. }));
        let current = fs::read(&file_path).expect("file should still exist");
        assert_eq!(current, b"embedded");
    }

    #[test]
    fn detects_supported_formats() {
        assert!(matches!(
            detect_tag_format(Path::new("track.mp3")).expect("mp3 format"),
            TagFormat::Id3v2
        ));
        assert!(matches!(
            detect_tag_format(Path::new("track.flac")).expect("flac format"),
            TagFormat::VorbisComments
        ));
        assert!(matches!(
            detect_tag_format(Path::new("track.m4a")).expect("m4a format"),
            TagFormat::Mp4
        ));
        assert!(matches!(
            detect_tag_format(Path::new("track.ape")).expect("ape format"),
            TagFormat::Ape
        ));
    }

    #[test]
    fn returns_clear_error_for_missing_extension() {
        let err = detect_tag_format(Path::new("trackname")).expect_err("should fail without extension");
        assert!(matches!(err, TagEmbeddingError::UnsupportedFormat(_)));
        assert!(err.to_string().contains("missing or invalid file extension"));
    }
}