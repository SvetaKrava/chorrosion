// SPDX-License-Identifier: GPL-3.0-or-later
//! Charset normalization and tag sanitation for embedded audio metadata.
//!
//! Tag values from external sources (MusicBrainz, Last.fm, filenames, etc.)
//! can arrive in inconsistent Unicode representations and may contain control
//! characters or extraneous whitespace that break downstream consumers.
//!
//! [`TagSanitizer`] applies a deterministic, lossless cleanup pipeline:
//! 1. Unicode NFC normalization — canonicalizes composed forms so that
//!    e.g. `e\u{301}` (e + combining acute) becomes `\u{e9}` (é).
//! 2. Control-character stripping — removes all ASCII C0 controls (U+0000–
//!    U+001F) and DEL (U+007F) as well as the Unicode soft-hyphen (U+00AD)
//!    which can silently alter string comparisons.
//! 3. Whitespace trimming — strips leading and trailing ASCII whitespace.
//! 4. Empty-after-sanitation guard — returns `None` for values that become
//!    empty after the above steps, so callers can skip writing blank tags.

use unicode_normalization::UnicodeNormalization as _;

use crate::tag_embedding::TagEmbeddingPayload;

// ============================================================================
// Sanitizer
// ============================================================================

/// Stateless utility for sanitizing text tag values.
pub struct TagSanitizer;

impl TagSanitizer {
    /// Sanitize a single tag text value.
    ///
    /// Returns `None` when the value is empty after cleanup, signalling that
    /// the tag should be omitted rather than written as blank.
    ///
    /// # Pipeline
    ///
    /// 1. **NFC normalization** via the `unicode-normalization` crate.
    /// 2. **Control-character removal** — strips U+0000–U+001F, U+007F, and
    ///    U+00AD (soft-hyphen).
    /// 3. **Whitespace trim** — removes leading and trailing ASCII whitespace.
    pub fn sanitize_text(value: &str) -> Option<String> {
        // 1. NFC-normalize.
        let normalized: String = value.nfc().collect();

        // 2. Strip control characters and soft-hyphen.
        let stripped: String = normalized
            .chars()
            .filter(|&c| !is_control_or_soft_hyphen(c))
            .collect();

        // 3. Trim whitespace.
        let trimmed = stripped.trim();

        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    /// Sanitize all string fields in a [`TagEmbeddingPayload`], returning a
    /// new payload with clean values.
    ///
    /// Fields that become empty after sanitation are set to `None`.
    pub fn sanitize_payload(payload: &TagEmbeddingPayload) -> TagEmbeddingPayload {
        TagEmbeddingPayload {
            artist: payload.artist.as_deref().and_then(Self::sanitize_text),
            album: payload.album.as_deref().and_then(Self::sanitize_text),
            title: payload.title.as_deref().and_then(Self::sanitize_text),
            // Numeric and binary fields are passed through unchanged.
            track_number: payload.track_number,
            disc_number: payload.disc_number,
            fingerprint_hash: payload
                .fingerprint_hash
                .as_deref()
                .and_then(Self::sanitize_text),
            artwork: payload.artwork.clone(),
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Returns `true` for characters that should be stripped from tag values:
/// - ASCII C0 control range U+0000–U+001F
/// - ASCII DEL U+007F
/// - Unicode soft-hyphen U+00AD (invisible, alters comparisons silently)
#[inline]
fn is_control_or_soft_hyphen(c: char) -> bool {
    matches!(c, '\u{0000}'..='\u{001F}' | '\u{007F}' | '\u{00AD}')
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag_embedding::{ArtworkData, TagEmbeddingPayload};

    // ---- sanitize_text ----

    #[test]
    fn passthrough_for_clean_ascii() {
        assert_eq!(
            TagSanitizer::sanitize_text("Boards of Canada").as_deref(),
            Some("Boards of Canada")
        );
    }

    #[test]
    fn normalizes_nfd_to_nfc() {
        // NFD: 'e' + combining acute accent → NFC: é (U+00E9)
        let nfd = "Cafe\u{0301}";
        let result = TagSanitizer::sanitize_text(nfd).unwrap();
        assert_eq!(result, "Caf\u{00E9}");
        // Result must be NFC (single codepoint for é).
        assert_eq!(result.len(), 5); // "Café" = 5 bytes in UTF-8, not 6
    }

    #[test]
    fn strips_null_bytes() {
        let with_null = "Artist\x00Name";
        assert_eq!(
            TagSanitizer::sanitize_text(with_null).as_deref(),
            Some("ArtistName")
        );
    }

    #[test]
    fn strips_ascii_control_characters() {
        // Tab, BEL, ESC embedded in value
        let with_controls = "Track\x07\x1B\tTitle";
        assert_eq!(
            TagSanitizer::sanitize_text(with_controls).as_deref(),
            Some("TrackTitle")
        );
    }

    #[test]
    fn strips_del_character() {
        let with_del = "Album\x7FTitle";
        assert_eq!(
            TagSanitizer::sanitize_text(with_del).as_deref(),
            Some("AlbumTitle")
        );
    }

    #[test]
    fn strips_soft_hyphen() {
        // U+00AD invisible soft-hyphen
        let with_soft_hyphen = "Mu\u{00AD}sic";
        assert_eq!(
            TagSanitizer::sanitize_text(with_soft_hyphen).as_deref(),
            Some("Music")
        );
    }

    #[test]
    fn trims_leading_and_trailing_whitespace() {
        assert_eq!(
            TagSanitizer::sanitize_text("  Roygbiv  ").as_deref(),
            Some("Roygbiv")
        );
    }

    #[test]
    fn trims_only_whitespace_returns_none() {
        assert!(TagSanitizer::sanitize_text("   ").is_none());
    }

    #[test]
    fn empty_string_returns_none() {
        assert!(TagSanitizer::sanitize_text("").is_none());
    }

    #[test]
    fn only_control_chars_returns_none() {
        assert!(TagSanitizer::sanitize_text("\x00\x01\x1F").is_none());
    }

    #[test]
    fn preserves_non_ascii_unicode_beyond_control_range() {
        // Characters above U+009F are preserved
        let value = "Sigur Rós";
        assert_eq!(
            TagSanitizer::sanitize_text(value).as_deref(),
            Some("Sigur Rós")
        );
    }

    // ---- sanitize_payload ----

    #[test]
    fn sanitize_payload_cleans_all_string_fields() {
        let payload = TagEmbeddingPayload {
            artist: Some("  Artist\x00  ".to_string()),
            album: Some("Album\x1F".to_string()),
            title: Some("\tTitle\t".to_string()),
            fingerprint_hash: Some("abc123".to_string()),
            track_number: Some(3),
            disc_number: Some(1),
            artwork: None,
        };

        let clean = TagSanitizer::sanitize_payload(&payload);
        assert_eq!(clean.artist.as_deref(), Some("Artist"));
        assert_eq!(clean.album.as_deref(), Some("Album"));
        assert_eq!(clean.title.as_deref(), Some("Title"));
        assert_eq!(clean.fingerprint_hash.as_deref(), Some("abc123"));
        assert_eq!(clean.track_number, Some(3));
        assert_eq!(clean.disc_number, Some(1));
        assert!(clean.artwork.is_none());
    }

    #[test]
    fn sanitize_payload_sets_none_for_empty_after_sanitation() {
        let payload = TagEmbeddingPayload {
            artist: Some("\x00\x01".to_string()),
            album: None,
            title: Some("Valid Title".to_string()),
            fingerprint_hash: None,
            track_number: None,
            disc_number: None,
            artwork: None,
        };

        let clean = TagSanitizer::sanitize_payload(&payload);
        assert!(clean.artist.is_none());
        assert!(clean.album.is_none());
        assert_eq!(clean.title.as_deref(), Some("Valid Title"));
    }

    #[test]
    fn sanitize_payload_preserves_artwork() {
        let artwork = ArtworkData {
            mime_type: "image/jpeg".to_string(),
            bytes: vec![0xFF, 0xD8, 0xFF],
        };
        let payload = TagEmbeddingPayload {
            artist: Some("Artist".to_string()),
            album: None,
            title: None,
            fingerprint_hash: None,
            track_number: None,
            disc_number: None,
            artwork: Some(artwork.clone()),
        };

        let clean = TagSanitizer::sanitize_payload(&payload);
        assert_eq!(clean.artwork, Some(artwork));
    }
}
