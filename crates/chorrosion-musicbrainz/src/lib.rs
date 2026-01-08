// SPDX-License-Identifier: GPL-3.0-or-later

//! MusicBrainz API client for fetching music metadata.
//!
//! This crate provides a client for interacting with the MusicBrainz API,
//! including artist and album search/lookup functionality with built-in
//! rate limiting to comply with MusicBrainz API guidelines.

pub mod client;
#[cfg(test)]
mod client_tests;
pub mod error;
pub mod models;
pub mod rate_limiter;

pub use client::MusicBrainzClient;
pub use error::{MusicBrainzError, Result};
pub use models::{
    Album, AlbumSearchResult, Artist, ArtistSearchResult, SearchQuery, SearchResponse,
};
