// SPDX-License-Identifier: GPL-3.0-or-later

//! Audio fingerprinting and AcoustID integration for music identification.
//!
//! This crate provides functionality for:
//! - Generating Chromaprint audio fingerprints
//! - Submitting fingerprints to AcoustID for identification
//! - Matching fingerprints to MusicBrainz recordings with confidence thresholds

pub mod acoustid;
pub mod error;
pub mod fingerprint;

pub use acoustid::{AcoustidClient, RecordingMatch};
pub use error::{FingerprintError, Result};
pub use fingerprint::Fingerprint;
