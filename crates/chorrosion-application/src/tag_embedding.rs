// SPDX-License-Identifier: GPL-3.0-or-later

use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, ItemValue, Tag, TagItem, TagType};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

use crate::tag_sanitation::TagSanitizer;

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

/// Controls whether existing embedded metadata and artwork are preserved or
/// replaced during import and refresh operations.
///
/// This is the primary user-facing preference for tag-write behaviour.
/// Read the derived flag via [`TagEmbeddingOptions::overwrite_existing`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmbeddedTagPreference {
    /// Always write catalog metadata and artwork, replacing whatever is
    /// already embedded in the file.  This is the default because chorrosion
    /// is authoritative over its own catalog.
    #[default]
    Overwrite,
    /// Skip any field or picture that is already present in the file.  Only
    /// fields that are completely absent will be written.  Useful when you
    /// want to keep manually curated artwork or custom comments untouched.
    Preserve,
}

#[derive(Debug, Clone)]
pub struct TagEmbeddingOptions {
    pub enabled: bool,
    pub read_only: bool,
    pub verify_roundtrip: bool,
    pub allowed_quality_names: Option<Vec<String>>,
    /// When `true` (the default), all text tag values are run through
    /// [`TagSanitizer`] before being written: Unicode NFC normalization,
    /// control-character stripping, and whitespace trimming.
    pub sanitize_text: bool,
    /// User-facing preference controlling whether existing embedded metadata
    /// and artwork are preserved or replaced.  Use [`Self::with_preference`]
    /// to build instances and [`Self::overwrite_existing`] to read the
    /// derived flag.
    pub preference: EmbeddedTagPreference,
}

impl Default for TagEmbeddingOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            read_only: false,
            verify_roundtrip: true,
            allowed_quality_names: None,
            sanitize_text: true,
            preference: EmbeddedTagPreference::Overwrite,
        }
    }
}

impl TagEmbeddingOptions {
    /// Returns `true` when existing embedded tags and artwork should be
    /// replaced; `false` when they should be preserved.
    ///
    /// This is a computed property derived from [`Self::preference`] and is
    /// the single source of truth used by the embedding logic.  To change the
    /// behaviour, set [`Self::preference`] via [`Self::with_preference`].
    #[must_use]
    pub fn overwrite_existing(&self) -> bool {
        matches!(self.preference, EmbeddedTagPreference::Overwrite)
    }

    /// Apply a user preference, returning an updated `TagEmbeddingOptions`.
    ///
    /// This is the canonical way to choose between preserving and overwriting
    /// existing embedded metadata.  The [`Self::overwrite_existing`] flag is
    /// always derived from this preference, so there is no risk of the two
    /// getting out of sync.
    ///
    /// ```rust,no_run
    /// # use chorrosion_application::tag_embedding::{TagEmbeddingOptions, EmbeddedTagPreference};
    /// let opts = TagEmbeddingOptions::default()
    ///     .with_preference(EmbeddedTagPreference::Preserve);
    /// assert!(!opts.overwrite_existing());
    /// assert_eq!(opts.preference, EmbeddedTagPreference::Preserve);
    /// ```
    #[must_use]
    pub fn with_preference(mut self, preference: EmbeddedTagPreference) -> Self {
        self.preference = preference;
        self
    }

    /// Options tuned for an initial import: overwrite any pre-existing tags so
    /// the catalog is authoritative from the start.
    #[must_use]
    pub fn for_import() -> Self {
        Self::default().with_preference(EmbeddedTagPreference::Overwrite)
    }

    /// Options tuned for a library refresh: overwrite tags so that corrections
    /// made in the catalog propagate back to the files.
    #[must_use]
    pub fn for_refresh() -> Self {
        Self::default().with_preference(EmbeddedTagPreference::Overwrite)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagEmbeddingOutcome {
    Embedded { format: TagFormat },
    Skipped { reason: String },
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

pub struct LoftyTagEmbeddingBackend;

impl LoftyTagEmbeddingBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LoftyTagEmbeddingBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl TagEmbeddingBackend for LoftyTagEmbeddingBackend {
    fn write_to_path(
        &self,
        file_path: &Path,
        format: TagFormat,
        payload: &TagEmbeddingPayload,
        overwrite_existing: bool,
    ) -> Result<(), String> {
        let mut tagged_file = Probe::open(file_path)
            .map_err(|err| err.to_string())?
            .read()
            .map_err(|err| err.to_string())?;

        let tag_type = format_to_tag_type(format);

        let mut working_tag = tagged_file
            .remove(tag_type)
            .unwrap_or_else(|| Tag::new(tag_type));

        if overwrite_existing || working_tag.artist().is_none() {
            set_optional_text(&mut working_tag, payload.artist.as_deref(), |tag, value| {
                tag.set_artist(value.to_string());
            });
        }
        if overwrite_existing || working_tag.album().is_none() {
            set_optional_text(&mut working_tag, payload.album.as_deref(), |tag, value| {
                tag.set_album(value.to_string());
            });
        }
        if overwrite_existing || working_tag.title().is_none() {
            set_optional_text(&mut working_tag, payload.title.as_deref(), |tag, value| {
                tag.set_title(value.to_string());
            });
        }

        if let Some(track_number) = payload.track_number {
            if overwrite_existing || working_tag.track().is_none() {
                working_tag.set_track(track_number);
            }
        }

        if let Some(disc_number) = payload.disc_number {
            if overwrite_existing || working_tag.disk().is_none() {
                working_tag.set_disk(disc_number);
            }
        }

        upsert_fingerprint_item(
            &mut working_tag,
            payload.fingerprint_hash.as_deref(),
            overwrite_existing,
        );

        upsert_artwork(
            &mut working_tag,
            payload.artwork.as_ref(),
            overwrite_existing,
        )
        .map_err(|err| err.to_string())?;

        tagged_file.insert_tag(working_tag);
        tagged_file
            .save_to_path(file_path, WriteOptions::default())
            .map_err(|err| err.to_string())
    }

    fn read_snapshot(
        &self,
        file_path: &Path,
        format: TagFormat,
    ) -> Result<TagRoundtripSnapshot, String> {
        let tagged_file = Probe::open(file_path)
            .map_err(|err| err.to_string())?
            .read()
            .map_err(|err| err.to_string())?;

        let tag_type = format_to_tag_type(format);
        let tag = tagged_file
            .tag(tag_type)
            .or_else(|| tagged_file.primary_tag())
            .ok_or_else(|| "no readable tags found for roundtrip".to_string())?;

        let fingerprint_hash = tag
            .get_string(&ItemKey::Unknown("CHORROSION_FINGERPRINT".to_string()))
            .map(|value| value.to_string());

        Ok(TagRoundtripSnapshot {
            artist: tag.artist().map(|value| value.to_string()),
            album: tag.album().map(|value| value.to_string()),
            title: tag.title().map(|value| value.to_string()),
            fingerprint_hash,
            has_artwork: !tag.pictures().is_empty(),
        })
    }
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
        if !quality_allowed(
            request.quality_name.as_deref(),
            options.allowed_quality_names.as_ref(),
        ) {
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

        // Sanitize payload text fields when the option is enabled.
        let sanitized_payload;
        let payload = if options.sanitize_text {
            sanitized_payload = TagSanitizer::sanitize_payload(request.payload.clone());
            &sanitized_payload
        } else {
            &request.payload
        };

        fs::copy(&request.file_path, &backup_path)
            .map_err(|err| TagEmbeddingError::FileOperation(err.to_string()))?;
        fs::copy(&request.file_path, &temp_path)
            .map_err(|err| TagEmbeddingError::FileOperation(err.to_string()))?;

        if let Err(error) =
            self.backend
                .write_to_path(&temp_path, format, payload, options.overwrite_existing())
        {
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

            if let Err(err) = verify_snapshot(payload, &snapshot) {
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

fn format_to_tag_type(format: TagFormat) -> TagType {
    match format {
        TagFormat::Id3v2 => TagType::Id3v2,
        TagFormat::VorbisComments => TagType::VorbisComments,
        TagFormat::Mp4 => TagType::Mp4Ilst,
        TagFormat::Ape => TagType::Ape,
    }
}

fn set_optional_text<F>(tag: &mut Tag, value: Option<&str>, mut setter: F)
where
    F: FnMut(&mut Tag, &str),
{
    if let Some(value) = value {
        setter(tag, value);
    }
}

fn upsert_fingerprint_item(
    tag: &mut Tag,
    fingerprint_hash: Option<&str>,
    overwrite_existing: bool,
) {
    let Some(fingerprint_hash) = fingerprint_hash else {
        return;
    };

    let key = ItemKey::Unknown("CHORROSION_FINGERPRINT".to_string());
    let existing = tag.get_string(&key);

    if existing.is_none() || overwrite_existing {
        // `Tag::insert` rejects ItemKey::Unknown (map_key returns None),
        // so use insert_unchecked which bypasses the key-mapping check.
        tag.insert_unchecked(TagItem::new(
            key,
            ItemValue::Text(fingerprint_hash.to_string()),
        ));
    }
}

fn upsert_artwork(
    tag: &mut Tag,
    artwork: Option<&ArtworkData>,
    overwrite_existing: bool,
) -> Result<(), String> {
    let Some(artwork) = artwork else {
        return Ok(());
    };

    let has_cover_front = tag
        .pictures()
        .iter()
        .any(|p| p.pic_type() == PictureType::CoverFront);

    if overwrite_existing && has_cover_front {
        tag.remove_picture_type(PictureType::CoverFront);
    } else if !overwrite_existing && has_cover_front {
        return Ok(());
    }

    let mime_type = MimeType::from_str(&artwork.mime_type);

    tag.push_picture(Picture::new_unchecked(
        PictureType::CoverFront,
        Some(mime_type),
        None,
        artwork.bytes.clone(),
    ));

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
        registry.lock().expect("TEMP_DIRS lock").push(dir);
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
        let err =
            detect_tag_format(Path::new("trackname")).expect_err("should fail without extension");
        assert!(matches!(err, TagEmbeddingError::UnsupportedFormat(_)));
        assert!(err
            .to_string()
            .contains("missing or invalid file extension"));
    }

    // ── upsert_artwork unit tests ─────────────────────────────────────────────

    fn make_artwork(bytes: Vec<u8>) -> ArtworkData {
        ArtworkData {
            mime_type: "image/jpeg".to_string(),
            bytes,
        }
    }

    #[test]
    fn upsert_artwork_no_op_when_none_provided() {
        let mut tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
        upsert_artwork(&mut tag, None, true).expect("should succeed");
        assert!(tag.pictures().is_empty());
    }

    #[test]
    fn upsert_artwork_adds_cover_front_when_tag_is_empty() {
        let mut tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
        let artwork = make_artwork(vec![0x01, 0x02, 0x03]);
        upsert_artwork(&mut tag, Some(&artwork), false).expect("should succeed");
        assert_eq!(tag.pictures().len(), 1);
        assert_eq!(tag.pictures()[0].pic_type(), PictureType::CoverFront);
        assert_eq!(tag.pictures()[0].data(), &[0x01, 0x02, 0x03]);
    }

    #[test]
    fn upsert_artwork_adds_cover_front_when_only_non_cover_front_exists() {
        let mut tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
        tag.push_picture(Picture::new_unchecked(
            PictureType::Artist,
            Some(MimeType::Jpeg),
            None,
            vec![0xAA, 0xBB],
        ));

        let artwork = make_artwork(vec![0x01, 0x02]);
        upsert_artwork(&mut tag, Some(&artwork), false).expect("should succeed");

        // Should now have both the artist picture and the new CoverFront
        assert_eq!(tag.pictures().len(), 2);
        assert!(tag
            .pictures()
            .iter()
            .any(|p| p.pic_type() == PictureType::CoverFront));
        assert!(tag
            .pictures()
            .iter()
            .any(|p| p.pic_type() == PictureType::Artist));
    }

    #[test]
    fn upsert_artwork_preserves_existing_cover_front_when_overwrite_false() {
        let mut tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
        tag.push_picture(Picture::new_unchecked(
            PictureType::CoverFront,
            Some(MimeType::Jpeg),
            None,
            vec![0x01, 0x02], // "original" bytes
        ));

        let artwork = make_artwork(vec![0x10, 0x20, 0x30]);
        upsert_artwork(&mut tag, Some(&artwork), false).expect("should succeed");

        assert_eq!(tag.pictures().len(), 1);
        assert_eq!(tag.pictures()[0].data(), &[0x01, 0x02]); // unchanged
    }

    #[test]
    fn upsert_artwork_replaces_cover_front_when_overwrite_true() {
        let mut tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
        tag.push_picture(Picture::new_unchecked(
            PictureType::CoverFront,
            Some(MimeType::Jpeg),
            None,
            vec![0x01, 0x02],
        ));

        let artwork = make_artwork(vec![0x10, 0x20, 0x30]);
        upsert_artwork(&mut tag, Some(&artwork), true).expect("should succeed");

        assert_eq!(tag.pictures().len(), 1);
        assert_eq!(tag.pictures()[0].data(), &[0x10, 0x20, 0x30]); // replaced
    }

    // ── LoftyTagEmbeddingBackend integration tests ────────────────────────────

    use crate::test_fixtures::{MINIMAL_FLAC, MINIMAL_MP3};

    fn write_fixture(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, bytes).expect("fixture write");
        path
    }

    fn full_payload() -> TagEmbeddingPayload {
        TagEmbeddingPayload {
            artist: Some("Roundtrip Artist".to_string()),
            album: Some("Roundtrip Album".to_string()),
            title: Some("Roundtrip Track".to_string()),
            track_number: Some(7),
            disc_number: Some(2),
            fingerprint_hash: Some("fp_deadbeef".to_string()),
            artwork: Some(ArtworkData {
                mime_type: "image/jpeg".to_string(),
                bytes: vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x01],
            }),
        }
    }

    #[test]
    fn lofty_backend_id3v2_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_fixture(&dir, "track.mp3", MINIMAL_MP3);

        let backend = LoftyTagEmbeddingBackend::new();
        backend
            .write_to_path(&path, TagFormat::Id3v2, &full_payload(), true)
            .expect("write should succeed");

        let snapshot = backend
            .read_snapshot(&path, TagFormat::Id3v2)
            .expect("read should succeed");

        assert_eq!(snapshot.artist.as_deref(), Some("Roundtrip Artist"));
        assert_eq!(snapshot.album.as_deref(), Some("Roundtrip Album"));
        assert_eq!(snapshot.title.as_deref(), Some("Roundtrip Track"));
        assert_eq!(snapshot.fingerprint_hash.as_deref(), Some("fp_deadbeef"));
        assert!(snapshot.has_artwork);
    }

    #[test]
    fn lofty_backend_vorbis_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_fixture(&dir, "track.flac", MINIMAL_FLAC);

        let backend = LoftyTagEmbeddingBackend::new();
        backend
            .write_to_path(&path, TagFormat::VorbisComments, &full_payload(), true)
            .expect("write should succeed");

        let snapshot = backend
            .read_snapshot(&path, TagFormat::VorbisComments)
            .expect("read should succeed");

        assert_eq!(snapshot.artist.as_deref(), Some("Roundtrip Artist"));
        assert_eq!(snapshot.album.as_deref(), Some("Roundtrip Album"));
        assert_eq!(snapshot.title.as_deref(), Some("Roundtrip Track"));
        assert_eq!(snapshot.fingerprint_hash.as_deref(), Some("fp_deadbeef"));
        assert!(snapshot.has_artwork);
    }

    #[test]
    fn lofty_backend_id3v2_overwrite_false_preserves_existing_text() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_fixture(&dir, "track_preserve.mp3", MINIMAL_MP3);

        let backend = LoftyTagEmbeddingBackend::new();

        // First write: establish initial tags
        let initial = TagEmbeddingPayload {
            artist: Some("Original Artist".to_string()),
            album: None,
            title: None,
            track_number: None,
            disc_number: None,
            fingerprint_hash: None,
            artwork: None,
        };
        backend
            .write_to_path(&path, TagFormat::Id3v2, &initial, true)
            .expect("initial write");

        // Second write with overwrite_existing=false must not replace artist
        let update = TagEmbeddingPayload {
            artist: Some("New Artist".to_string()),
            album: Some("New Album".to_string()),
            title: None,
            track_number: None,
            disc_number: None,
            fingerprint_hash: None,
            artwork: None,
        };
        backend
            .write_to_path(&path, TagFormat::Id3v2, &update, false)
            .expect("update write");

        let snapshot = backend
            .read_snapshot(&path, TagFormat::Id3v2)
            .expect("read");

        assert_eq!(snapshot.artist.as_deref(), Some("Original Artist")); // preserved
        assert_eq!(snapshot.album.as_deref(), Some("New Album")); // filled in (was absent)
    }

    // ── EmbeddedTagPreference / builder tests ─────────────────────────────────

    #[test]
    fn default_preference_is_overwrite() {
        let opts = TagEmbeddingOptions::default();
        assert_eq!(opts.preference, EmbeddedTagPreference::Overwrite);
        assert!(opts.overwrite_existing());
    }

    #[test]
    fn with_preference_preserve_clears_overwrite_existing() {
        let opts = TagEmbeddingOptions::default().with_preference(EmbeddedTagPreference::Preserve);
        assert_eq!(opts.preference, EmbeddedTagPreference::Preserve);
        assert!(!opts.overwrite_existing());
    }

    #[test]
    fn with_preference_overwrite_sets_overwrite_existing() {
        let base = TagEmbeddingOptions::default().with_preference(EmbeddedTagPreference::Preserve);
        let opts = base.with_preference(EmbeddedTagPreference::Overwrite);
        assert_eq!(opts.preference, EmbeddedTagPreference::Overwrite);
        assert!(opts.overwrite_existing());
    }

    #[test]
    fn for_import_uses_overwrite() {
        let opts = TagEmbeddingOptions::for_import();
        assert_eq!(opts.preference, EmbeddedTagPreference::Overwrite);
        assert!(opts.overwrite_existing());
        assert!(opts.enabled);
    }

    #[test]
    fn for_refresh_uses_overwrite() {
        let opts = TagEmbeddingOptions::for_refresh();
        assert_eq!(opts.preference, EmbeddedTagPreference::Overwrite);
        assert!(opts.overwrite_existing());
        assert!(opts.enabled);
    }

    #[test]
    fn with_preference_preserves_other_fields() {
        let opts = TagEmbeddingOptions {
            read_only: true,
            verify_roundtrip: false,
            sanitize_text: false,
            ..TagEmbeddingOptions::default()
        }
        .with_preference(EmbeddedTagPreference::Preserve);

        assert!(opts.read_only);
        assert!(!opts.verify_roundtrip);
        assert!(!opts.sanitize_text);
        assert_eq!(opts.preference, EmbeddedTagPreference::Preserve);
    }
}
