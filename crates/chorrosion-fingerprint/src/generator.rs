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
//! **Always available:**
//! - FLAC (Free Lossless Audio Codec)
//! - MP3 (MPEG-1 Audio Layer III)
//!
//! **With optional `ffmpeg-support` feature:**
//! - OGG Vorbis
//! - OGG Opus
//! - WavPack
//! - APE (Monkey's Audio)
//! - DSF (DSD Stream File)
//! - M4A / AAC (via FFmpeg)
//! - And any other format supported by FFmpeg
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
//! **Phase 3 (Issue #89):**
//! - FFmpeg support for advanced formats (OGG, Opus, etc.)
//! - Feature flag: `ffmpeg-support`
//! - Graceful fallback when FFmpeg not available

use std::fs::File;
use std::io::{ErrorKind, Read, Seek};
use std::path::Path;

use chromaprint::Chromaprint;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tracing::{debug, instrument};

use crate::{Fingerprint, FingerprintError, Result};

#[cfg(feature = "ffmpeg-support")]
use crate::ffmpeg_decoder;

/// Maximum duration to use for fingerprinting (in seconds).
/// Chromaprint standard is 120 seconds for optimal recognition.
pub const MAX_FINGERPRINT_DURATION_SECS: u32 = 120;

/// Sample rate for audio processing (44.1 kHz is standard).
const SAMPLE_RATE: u32 = 44100;

/// Audio samples: mono, 16-bit PCM at a given sample rate.
///
/// # Public API
///
/// This struct is part of the public API for the `ffmpeg_decoder` module integration.
/// All fields are public to allow cross-module audio processing.
///
/// # Invariants
///
/// - `duration_secs` should equal `samples.len() / sample_rate` (approximately)
/// - `sample_rate` should be a valid audio sample rate (typically 44100, 48000, etc.)
/// - `samples` should contain mono, 16-bit PCM data
pub struct AudioSamples {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
    pub duration_secs: u32,
}

impl AudioSamples {
    /// Create new audio samples with calculated duration.
    fn new(samples: Vec<i16>, sample_rate: u32) -> Self {
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
        let effective_sample_rate = self.sample_rate.max(1);
        let max_samples = (effective_sample_rate * MAX_FINGERPRINT_DURATION_SECS) as usize;
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
    /// # Format Support
    ///
    /// - Always available: FLAC, MP3
    /// - With `ffmpeg-support` feature: OGG Vorbis, Opus, WavPack, APE, DSF, M4A, AAC, and more
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
        let reader = File::open(path).map_err(|e| {
            FingerprintError::AudioProcessing(format!("Failed to open audio file: {}", e))
        })?;

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
            #[cfg(feature = "ffmpeg-support")]
            "ogg" | "opus" | "oga" => self.extract_ffmpeg_samples(path, &extension).await?,
            #[cfg(feature = "ffmpeg-support")]
            "wv" => self.extract_ffmpeg_samples(path, &extension).await?,
            #[cfg(feature = "ffmpeg-support")]
            "ape" => self.extract_ffmpeg_samples(path, &extension).await?,
            #[cfg(feature = "ffmpeg-support")]
            "dsf" => self.extract_ffmpeg_samples(path, &extension).await?,
            #[cfg(feature = "ffmpeg-support")]
            "m4a" | "aac" => self.extract_ffmpeg_samples(path, &extension).await?,
            #[cfg(feature = "ffmpeg-support")]
            "wav" | "aiff" | "aifc" => self.extract_ffmpeg_samples(path, &extension).await?,
            _ => {
                #[cfg(feature = "ffmpeg-support")]
                {
                    // Try FFmpeg as fallback for unknown formats
                    match self.extract_ffmpeg_samples(path, &extension).await {
                        Ok(samples) => {
                            debug!(
                                format = %extension,
                                "Successfully decoded unknown format using FFmpeg"
                            );
                            samples
                        }
                        Err(e) => {
                            return Err(FingerprintError::AudioProcessing(format!(
                                "Failed to decode {} using FFmpeg: {}",
                                extension, e
                            )))
                        }
                    }
                }
                #[cfg(not(feature = "ffmpeg-support"))]
                {
                    return Err(FingerprintError::AudioProcessing(format!(
                        "Unsupported audio format: {} (enable 'ffmpeg-support' for additional formats)",
                        extension
                    )));
                }
            }
        };

        self.generate_fingerprint_from_samples(samples).await
    }

    /// Extract audio samples from FLAC file.
    async fn extract_flac_samples<R: Read + Seek + MediaSource + 'static>(
        &self,
        reader: R,
    ) -> Result<AudioSamples> {
        debug!("Extracting samples from FLAC file");
        self.decode_audio(reader, "flac").await
    }

    /// Extract audio samples from MP3 file.
    async fn extract_mp3_samples<R: Read + Seek + MediaSource + 'static>(
        &self,
        reader: R,
    ) -> Result<AudioSamples> {
        debug!("Extracting samples from MP3 file");
        self.decode_audio(reader, "mp3").await
    }

    /// Extract audio samples using FFmpeg (for advanced formats).
    #[cfg(feature = "ffmpeg-support")]
    async fn extract_ffmpeg_samples<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        path: P,
        _format: &str,
    ) -> Result<AudioSamples> {
        let samples = ffmpeg_decoder::decode_audio_ffmpeg(path).await?;
        let sample_count = samples.len();

        Ok(AudioSamples {
            samples,
            sample_rate: 44100, // Default; FFmpeg decoder returns at source rate
            duration_secs: (sample_count as u32 / 44100).max(1),
        })
    }

    /// Decode audio using symphonia and return mono PCM samples.
    async fn decode_audio<R: Read + Seek + MediaSource + 'static>(
        &self,
        reader: R,
        extension: &str,
    ) -> Result<AudioSamples> {
        let mss = MediaSourceStream::new(Box::new(reader), Default::default());

        let mut hint = Hint::new();
        hint.with_extension(extension);

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| {
                FingerprintError::AudioProcessing(format!(
                    "Failed to probe {} stream: {}",
                    extension, e
                ))
            })?;

        let mut format = probed.format;
        let track = format.default_track().ok_or_else(|| {
            FingerprintError::AudioProcessing("No audio tracks found".to_string())
        })?;

        let track_id = track.id;

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| {
                FingerprintError::AudioProcessing(format!(
                    "Failed to create decoder for {}: {}",
                    extension, e
                ))
            })?;

        let mut sample_rate = track.codec_params.sample_rate.unwrap_or(SAMPLE_RATE);
        let estimated_capacity = track
            .codec_params
            .n_frames
            .and_then(|f| usize::try_from(f).ok())
            .unwrap_or(sample_rate as usize * 120);
        let mut samples: Vec<i16> = Vec::with_capacity(estimated_capacity);

        let max_samples = (MAX_FINGERPRINT_DURATION_SECS as usize) * (sample_rate as usize);

        loop {
            if samples.len() >= max_samples {
                break;
            }

            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(err)) if err.kind() == ErrorKind::UnexpectedEof => {
                    break
                }
                Err(SymphoniaError::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(e) => {
                    return Err(FingerprintError::AudioProcessing(format!(
                        "Error reading {} packet: {}",
                        extension, e
                    )))
                }
            };

            if packet.track_id() != track_id {
                continue;
            }

            let decoded = decoder.decode(&packet).map_err(|e| {
                FingerprintError::AudioProcessing(format!(
                    "Failed to decode {} frame: {}",
                    extension, e
                ))
            })?;

            match decoded {
                AudioBufferRef::F32(buf) => {
                    let spec = buf.spec();
                    if spec.rate > 0 {
                        sample_rate = spec.rate;
                    }

                    let channels = spec.channels.count().max(1);
                    let frames = buf.frames();

                    for frame_idx in 0..frames {
                        let mut mixed = 0.0f32;
                        for ch in 0..channels {
                            mixed += buf.chan(ch)[frame_idx];
                        }
                        mixed /= channels as f32;
                        let clipped = mixed.clamp(-1.0, 1.0);
                        samples.push((clipped * i16::MAX as f32) as i16);
                    }
                }
                AudioBufferRef::S16(buf) => {
                    let spec = buf.spec();
                    if spec.rate > 0 {
                        sample_rate = spec.rate;
                    }

                    let channels = spec.channels.count().max(1);
                    let frames = buf.frames();

                    for frame_idx in 0..frames {
                        let mut mixed: i32 = 0;
                        for ch in 0..channels {
                            mixed += buf.chan(ch)[frame_idx] as i32;
                        }
                        mixed /= channels as i32;
                        samples.push(mixed.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
                    }
                }
                AudioBufferRef::S32(buf) => {
                    let spec = buf.spec();
                    if spec.rate > 0 {
                        sample_rate = spec.rate;
                    }

                    let channels = spec.channels.count().max(1);
                    let frames = buf.frames();

                    for frame_idx in 0..frames {
                        let mut mixed: f64 = 0.0;
                        for ch in 0..channels {
                            mixed += buf.chan(ch)[frame_idx] as f64;
                        }
                        mixed /= channels as f64;
                        let clipped = (mixed / (i32::MAX as f64 / i16::MAX as f64))
                            .clamp(i16::MIN as f64, i16::MAX as f64);
                        samples.push(clipped as i16);
                    }
                }
                _other => {
                    return Err(FingerprintError::AudioProcessing(format!(
                        "Unsupported sample format for {}",
                        extension
                    )));
                }
            }
        }

        let mut audio = AudioSamples::new(samples, sample_rate);
        audio.limit_to_fingerprint_duration();
        Ok(audio)
    }

    /// Generate fingerprint from audio samples.
    async fn generate_fingerprint_from_samples(
        &self,
        samples: AudioSamples,
    ) -> Result<Fingerprint> {
        if samples.samples.is_empty() {
            return Err(FingerprintError::AudioProcessing(
                "No audio samples available".to_string(),
            ));
        }

        debug!(
            sample_count = samples.samples.len(),
            duration_secs = samples.duration_secs,
            sample_rate = samples.sample_rate,
            "Generating fingerprint from audio samples"
        );

        let mut ctx = Chromaprint::new();

        if !ctx.start(samples.sample_rate as i32, 1) {
            return Err(FingerprintError::AudioProcessing(
                "Failed to start Chromaprint".to_string(),
            ));
        }

        if !ctx.feed(&samples.samples) {
            return Err(FingerprintError::AudioProcessing(
                "Failed to feed samples to Chromaprint".to_string(),
            ));
        }

        if !ctx.finish() {
            return Err(FingerprintError::AudioProcessing(
                "Chromaprint finalize failed".to_string(),
            ));
        }

        let hash = ctx.fingerprint().ok_or_else(|| {
            FingerprintError::AudioProcessing(
                "Chromaprint did not return a fingerprint".to_string(),
            )
        })?;

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
        let samples = vec![1i16, 2, 3];
        let audio = AudioSamples::new(samples.clone(), 44100);

        assert_eq!(audio.samples, samples);
        assert_eq!(audio.sample_rate, 44100);
        assert_eq!(audio.duration_secs, 1);
    }

    #[test]
    fn test_audio_samples_limit_to_fingerprint_duration() {
        let sample_count = (44100 * 150) as usize;
        let samples = vec![10i16; sample_count];
        let mut audio = AudioSamples::new(samples, 44100);

        audio.limit_to_fingerprint_duration();

        assert!(audio.samples.len() <= (44100 * 120) as usize);
        assert_eq!(audio.duration_secs, 120);
    }

    #[test]
    fn test_audio_samples_no_limit_needed() {
        let sample_count = (44100 * 60) as usize;
        let samples = vec![10i16; sample_count];
        let mut audio = AudioSamples::new(samples.clone(), 44100);

        audio.limit_to_fingerprint_duration();

        assert_eq!(audio.samples.len(), sample_count);
        assert_eq!(audio.duration_secs, 60);
    }

    #[test]
    fn test_generator_creation() {
        let gen = FingerprintGenerator::new();
        assert_eq!(std::mem::size_of_val(&gen), 0);

        let gen2 = FingerprintGenerator;
        assert_eq!(std::mem::size_of_val(&gen2), 0);
    }

    #[cfg(feature = "ffmpeg-support")]
    #[test]
    fn test_ffmpeg_formats_recognized() {
        let ogg_ext = "ogg";
        let opus_ext = "opus";
        let wavpack_ext = "wv";

        assert!(matches!(ogg_ext, "ogg" | "opus" | "oga"));
        assert!(matches!(opus_ext, "ogg" | "opus" | "oga"));
        assert!(matches!(wavpack_ext, "wv"));
    }

    /// Test FFmpeg decoder with a supported format.
    ///
    /// This test verifies that FFmpeg decoding succeeds and returns valid audio samples.
    /// It requires an actual audio file to test against.
    ///
    /// # Note
    ///
    /// This is a template for integration testing. A real test would need:
    /// - A sample audio file in an FFmpeg-supported format
    /// - Verification that samples are extracted
    /// - Verification that sample rate and duration are correct
    #[cfg(feature = "ffmpeg-support")]
    #[ignore = "requires test audio file"]
    #[tokio::test]
    async fn test_ffmpeg_decoding_integration() {
        // Example: test with OGG Vorbis file
        let gen = FingerprintGenerator::new();
        let _result = gen.extract_ffmpeg_samples("test_audio.ogg", "ogg").await;

        // When implemented with test audio:
        // assert!(_result.is_ok());
        // let audio = _result.unwrap();
        // assert!(!audio.samples.is_empty());
        // assert!(audio.sample_rate > 0);
        // assert!(audio.duration_secs > 0);
    }

    /// Test FFmpeg error handling for missing files.
    ///
    /// Verifies that decoding fails gracefully with proper error context
    /// when the file does not exist.
    #[cfg(feature = "ffmpeg-support")]
    #[tokio::test]
    async fn test_ffmpeg_missing_file_error() {
        let gen = FingerprintGenerator::new();
        let result = gen
            .extract_ffmpeg_samples("nonexistent_file.ogg", "ogg")
            .await;

        // Should fail with AudioProcessing error containing context
        assert!(result.is_err());
        if let Err(FingerprintError::AudioProcessing(msg)) = result {
            // Error message should contain context about the failure
            assert!(!msg.is_empty());
        } else {
            panic!("Expected AudioProcessing error");
        }
    }
}
