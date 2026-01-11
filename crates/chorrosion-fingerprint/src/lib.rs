// SPDX-License-Identifier: GPL-3.0-or-later

//! Audio fingerprinting and AcoustID integration for music identification.
//!
//! This crate provides functionality for:
//! - Generating Chromaprint audio fingerprints from FLAC and MP3 files
//! - Submitting fingerprints to AcoustID for identification
//! - Matching fingerprints to MusicBrainz recordings with confidence thresholds

pub mod acoustid;
pub mod error;
pub mod fingerprint;
pub mod generator;

pub use acoustid::{AcoustidClient, RecordingArtist, RecordingMatch, ReleaseInfo};
pub use error::{FingerprintError, Result};
pub use fingerprint::Fingerprint;
pub use generator::FingerprintGenerator;
