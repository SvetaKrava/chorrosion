// SPDX-License-Identifier: GPL-3.0-or-later

use thiserror::Error;

pub type Result<T> = std::result::Result<T, MusicBrainzError>;

#[derive(Debug, Error)]
pub enum MusicBrainzError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid response from MusicBrainz API: {0}")]
    InvalidResponse(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
