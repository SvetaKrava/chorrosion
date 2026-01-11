// SPDX-License-Identifier: GPL-3.0-or-later

use thiserror::Error;

pub type Result<T> = std::result::Result<T, FingerprintError>;

#[derive(Debug, Error)]
pub enum FingerprintError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Audio processing error: {0}")]
    AudioProcessing(String),

    /// TODO: Use this when audio metadata extraction is implemented for fingerprinting.
    #[error("Failed to extract audio metadata: {0}")]
    AudioMetadataError(String),

    #[error("Invalid fingerprint: {0}")]
    InvalidFingerprint(String),

    #[error("AcoustID API error: {0}")]
    AcoustidError(String),

    #[error("Low confidence match (score: {score})")]
    LowConfidence { score: f32 },

    #[error("Invalid response from AcoustID API: {0}")]
    InvalidResponse(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
