// SPDX-License-Identifier: GPL-3.0-or-later

//! Audio fingerprint generation from various audio file formats.
//!
//! This module provides functionality to:
//! - Extract audio samples from FLAC, MP3, and other formats
//! - Generate Chromaprint fingerprints from audio samples
//! - Support audio duration extraction
//!
//! # Supported Formats
//!
//! - FLAC (Free Lossless Audio Codec)
//! - MP3 (MPEG-1 Audio Layer III)
//!
//! # Example
//!
//! ```no_run
//! use chorrosion_fingerprint::generator::FingerprintGenerator;
//! use std::path::Path;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let generator = FingerprintGenerator::new();
//! let fingerprint = generator.generate_from_file(Path::new("song.flac")).await?;
//! println!("Generated fingerprint with duration: {}s", fingerprint.duration);
//! # Ok(())
//! # }
//! ```
//!
//! # Implementation Status
//!
//! This is a phased implementation:
//!
//! **Phase 1 (Current):**
//! - Module structure and error handling
//! - File format detection
//! - AudioSamples container with duration limiting
//!
//! **Phase 2 (Issue #65.2):**
//! - FLAC audio decoding with symphonia
//! - MP3 audio decoding with symphonia
//! - Chromaprint fingerprint generation
//!
//! **Phase 3 (Issue #89 - Optional):**
//! - FFmpeg support for advanced formats (OGG, Opus, etc.)
//! - Feature flag: `ffmpeg-support`

use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use tracing::{debug, instrument, warn};

use crate::{Fingerprint, FingerprintError, Result};

/// Maximum duration to use for fingerprinting (in seconds).
/// Chromaprint standard is 120 seconds for optimal recognition.
const MAX_FINGERPRINT_DURATION_SECS: u32 = 120;

/// Sample rate for audio processing (44.1 kHz is standard).
const SAMPLE_RATE: u32 = 44100;

/// Audio samples: mono samples (f32) at 44.1 kHz.
struct AudioSamples {
    samples: Vec<f32>,
    #[allow(dead_code)] // Used in Phase 2 for actual audio decoding
    sample_rate: u32,
    duration_secs: u32,
}

impl AudioSamples {
    /// Create new audio samples with calculated duration.
    fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        let duration_secs = if samples.is_empty() {
            0
        } else {
            (samples.len() as u32 / sample_rate).max(1)
        };

        Self {
            samples,
            sample_rate,
            duration_secs,
        }
    }

    /// Limit samples to fingerprinting duration (120 seconds max).
    fn limit_to_fingerprint_duration(&mut self) {
        let max_samples = (SAMPLE_RATE * MAX_FINGERPRINT_DURATION_SECS) as usize;
        if self.samples.len() > max_samples {
            debug!(
                original_len = self.samples.len(),
                max_samples, "Truncating audio samples to fingerprint duration limit"
            );
            self.samples.truncate(max_samples);
            self.duration_secs = MAX_FINGERPRINT_DURATION_SECS;
        }
    }
}

/// Fingerprint generator for audio files.
pub struct FingerprintGenerator;

impl FingerprintGenerator {
    /// Create a new fingerprint generator.
    pub fn new() -> Self {
        Self
    }

    /// Generate fingerprint from an audio file.
    ///
    /// This method extracts audio from the specified file, converts it to a suitable
    /// format, and generates a Chromaprint fingerprint.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be opened or read
    /// - The audio format is not supported
    /// - Audio decoding fails
    /// - Fingerprinting fails
    #[instrument(skip(self), fields(file = ?path.as_ref()))]
    pub async fn generate_from_file<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        path: P,
    ) -> Result<Fingerprint> {
        let path = path.as_ref();

        debug!("Opening audio file for fingerprinting");
        let file = File::open(path).map_err(|e| {
            FingerprintError::AudioProcessing(format!("Failed to open audio file: {}", e))
        })?;

        let reader = BufReader::new(file);

        // Detect format from file extension
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_lowercase())
            .ok_or_else(|| {
                FingerprintError::AudioProcessing("Cannot determine audio format".to_string())
            })?;

        let samples = match extension.as_str() {
            "flac" => self.extract_flac_samples(reader).await?,
            "mp3" => self.extract_mp3_samples(reader).await?,
            _ => {
                return Err(FingerprintError::AudioProcessing(format!(
                    "Unsupported audio format: {}",
                    extension
                )))
            }
        };

        self.generate_fingerprint_from_samples(samples).await
    }

    /// Extract audio samples from FLAC file.
    ///
    /// TODO: Implement using symphonia FLAC decoder in Phase 2.
    async fn extract_flac_samples<R: Read + Seek>(&self, _reader: R) -> Result<AudioSamples> {
        debug!("Extracting samples from FLAC file");

        warn!("FLAC decoding not yet implemented in Phase 1; use placeholder fingerprint");

        // TODO Phase 2: Implement FLAC decoding with symphonia
        // let mut probe = symphonia::default::get_probe();
        // let source = Box::new(reader);
        // let probed = probe.instantiate(source)?;
        // let reader = probed.format;
        // let track = reader.default_track()?;
        // let mut decoder = symphonia::default::get_codec_registry().make(track.codec_params)?;
        // Then iterate over frames and collect samples

        let samples = vec![0.0f32; 44100 * 2]; // Placeholder: 2 seconds of silence
        Ok(AudioSamples::new(samples, SAMPLE_RATE))
    }

    /// Extract audio samples from MP3 file.
    ///
    /// TODO: Implement using symphonia MP3 decoder in Phase 2.
    async fn extract_mp3_samples<R: Read + Seek>(&self, _reader: R) -> Result<AudioSamples> {
        debug!("Extracting samples from MP3 file");

        warn!("MP3 decoding not yet implemented in Phase 1; use placeholder fingerprint");

        // TODO Phase 2: Implement MP3 decoding with symphonia
        // Similar to FLAC decoder above

        let samples = vec![0.0f32; 44100 * 2]; // Placeholder: 2 seconds of silence
        Ok(AudioSamples::new(samples, SAMPLE_RATE))
    }

    /// Generate fingerprint from audio samples.
    ///
    /// TODO: Implement using rusty-chromaprint in Phase 2.
    async fn generate_fingerprint_from_samples(
        &self,
        mut samples: AudioSamples,
    ) -> Result<Fingerprint> {
        if samples.samples.is_empty() {
            return Err(FingerprintError::AudioProcessing(
                "No audio samples available".to_string(),
            ));
        }

        debug!(
            sample_count = samples.samples.len(),
            duration_secs = samples.duration_secs,
            "Generating fingerprint from audio samples"
        );

        // Limit to fingerprinting duration (120 seconds max)
        samples.limit_to_fingerprint_duration();

        // TODO Phase 2: Implement Chromaprint generation
        // let mut printer = FingerprintPrinter::new(samples.sample_rate);
        // printer.feed(&samples.samples)?;
        // let hash = printer.finish()?;

        // For Phase 1, generate a deterministic placeholder based on sample hash
        warn!("Chromaprint generation not yet implemented in Phase 1; using placeholder");
        let hash = format!("PLACEHOLDER_{:x}", samples.samples.len());

        debug!(
            fingerprint_hash = %hash,
            "Generated placeholder fingerprint"
        );

        Fingerprint::new(hash, samples.duration_secs)
    }
}

impl Default for FingerprintGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_samples_creation() {
        let samples = vec![0.1f32, 0.2, 0.3];
        let audio = AudioSamples::new(samples.clone(), 44100);

        assert_eq!(audio.samples, samples);
        assert_eq!(audio.sample_rate, 44100);
        assert_eq!(audio.duration_secs, 1);
    }

    #[test]
    fn test_audio_samples_limit_to_fingerprint_duration() {
        let sample_count = (44100 * 150) as usize; // 150 seconds of audio
        let samples = vec![0.1f32; sample_count];
        let mut audio = AudioSamples::new(samples, 44100);

        audio.limit_to_fingerprint_duration();

        assert!(audio.samples.len() <= (44100 * 120) as usize);
        assert_eq!(audio.duration_secs, 120);
    }

    #[test]
    fn test_audio_samples_no_limit_needed() {
        let sample_count = (44100 * 60) as usize; // 60 seconds of audio
        let samples = vec![0.1f32; sample_count];
        let mut audio = AudioSamples::new(samples.clone(), 44100);

        audio.limit_to_fingerprint_duration();

        assert_eq!(audio.samples.len(), sample_count);
        assert_eq!(audio.duration_secs, 60);
    }

    #[test]
    fn test_generator_creation() {
        let gen = FingerprintGenerator::new();
        assert_eq!(std::mem::size_of_val(&gen), 0); // ZST

        let gen_default = FingerprintGenerator::default();
        assert_eq!(std::mem::size_of_val(&gen_default), 0);
    }

    #[test]
    fn test_unsupported_format_error() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let gen = FingerprintGenerator::new();
            let result = gen.generate_from_file("/nonexistent/path/file.xyz").await;

            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            // Could be either "Failed to open" or "Unsupported audio format" depending on path handling
            assert!(
                err_msg.contains("Failed to open") || err_msg.contains("Unsupported audio format"),
                "Unexpected error: {}",
                err_msg
            );
        });
    }
}
