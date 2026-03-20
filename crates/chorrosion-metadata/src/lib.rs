//! Library entry point for chorrosion-metadata

pub mod cover_art_fallback;
pub mod discogs;
pub mod fanarttv;
pub mod http_retry;
pub mod lastfm;
pub mod lyrics;

/// Default request timeout (in seconds) applied to HTTP clients when no explicit timeout is given.
pub(crate) const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 15;
