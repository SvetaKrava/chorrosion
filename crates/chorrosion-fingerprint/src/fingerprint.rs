// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

/// Audio fingerprint (Chromaprint).
///
/// A Chromaprint is a compressed audio fingerprint that can uniquely identify
/// a track regardless of quality, bitrate, or encoding. The fingerprint is
/// typically generated from the first 120 seconds of audio.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fingerprint {
    /// Chromaprint fingerprint hash (base64-encoded).
    pub hash: String,
    /// Duration of audio used to generate the fingerprint (in seconds).
    pub duration: u32,
    /// Algorithm version used (typically 4 for modern Chromaprint).
    #[serde(default = "default_algorithm")]
    pub algorithm: u32,
}

fn default_algorithm() -> u32 {
    4
}

impl Fingerprint {
    /// Create a new fingerprint.
    pub fn new(hash: impl Into<String>, duration: u32) -> Self {
        Self {
            hash: hash.into(),
            duration,
            algorithm: 4,
        }
    }

    /// Validate the fingerprint format.
    ///
    /// A valid Chromaprint fingerprint is a non-empty string containing
    /// base64-encoded data with a minimum duration > 0.
    pub fn validate(&self) -> crate::Result<()> {
        if self.hash.is_empty() {
            return Err(crate::FingerprintError::InvalidFingerprint(
                "fingerprint hash is empty".to_string(),
            ));
        }

        if self.duration == 0 {
            return Err(crate::FingerprintError::InvalidFingerprint(
                "duration must be > 0".to_string(),
            ));
        }

        // Chromaprint hashes are base64-encoded: alphanumeric, +, /, with = only as padding at the end
        let trimmed = self.hash.trim_end_matches('=');
        
        // Validate base64 padding length (0, 1, or 2 '=' characters allowed)
        let padding_len = self.hash.len() - trimmed.len();
        if padding_len > 2 {
            return Err(crate::FingerprintError::InvalidFingerprint(
                "invalid base64 padding: too many '=' characters".to_string(),
            ));
        }
        
        // Ensure = only appears at the end by checking if trimmed portion contains =
        if trimmed.contains('=') {
            return Err(crate::FingerprintError::InvalidFingerprint(
                "padding character '=' must only appear at the end".to_string(),
            ));
        }
        
        // Validate characters in the non-padding portion
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/')
        {
            return Err(crate::FingerprintError::InvalidFingerprint(
                "fingerprint contains invalid characters".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_creation() {
        let fp = Fingerprint::new("AQADvEWZ==", 120);
        assert_eq!(fp.hash, "AQADvEWZ==");
        assert_eq!(fp.duration, 120);
        assert_eq!(fp.algorithm, 4);
    }

    #[test]
    fn test_fingerprint_validation_valid() {
        let fp = Fingerprint::new("AQADvEWZ==", 120);
        assert!(fp.validate().is_ok());
    }

    #[test]
    fn test_fingerprint_validation_empty_hash() {
        let fp = Fingerprint::new("", 120);
        assert!(fp.validate().is_err());
    }

    #[test]
    fn test_fingerprint_validation_zero_duration() {
        let fp = Fingerprint::new("AQADvEWZ==", 0);
        assert!(fp.validate().is_err());
    }

    #[test]
    fn test_fingerprint_validation_invalid_chars() {
        let fp = Fingerprint::new("AQADv!WZ==", 120);
        assert!(fp.validate().is_err());
    }

    #[test]
    fn test_fingerprint_validation_padding_in_middle() {
        // = should only appear at the end, not in the middle
        let fp = Fingerprint::new("AQAD=vEWZ", 120);
        assert!(fp.validate().is_err());
    }

    #[test]
    fn test_fingerprint_validation_valid_no_padding() {
        // Valid base64 without padding
        let fp = Fingerprint::new("AQADvEWZ", 120);
        assert!(fp.validate().is_ok());
    }

    #[test]
    fn test_fingerprint_validation_valid_single_padding() {
        // Valid base64 with single = padding
        let fp = Fingerprint::new("AQADvEWZ=", 120);
        assert!(fp.validate().is_ok());
    }

    #[test]
    fn test_fingerprint_validation_excessive_padding() {
        // Invalid: too many padding characters (more than 2)
        let fp = Fingerprint::new("AQADvEWZ===", 120);
        assert!(fp.validate().is_err());
    }
}
