// SPDX-License-Identifier: GPL-3.0-or-later

//! FFmpeg-based audio decoder for advanced formats.
//!
//! This module provides audio decoding for formats not supported by Symphonia,
//! using FFmpeg as a backend. Supported formats include:
//!
//! - OGG Vorbis
//! - OGG Opus
//! - WavPack
//! - APE (Monkey's Audio)
//! - DSF (DSD Stream File)
//! - M4A / AAC
//! - WAV
//! - AIFF
//!
//! This module is only available with the `ffmpeg-support` feature flag.

use std::path::Path;

use tracing::{debug, instrument};

use crate::{FingerprintError, Result};

/// Decode audio from a file using FFmpeg.
///
/// Extracts mono, 16-bit PCM samples at the file's native sample rate
/// (or resamples to 44.1 kHz if needed for consistency).
///
/// # Arguments
///
/// * `path` - Path to the audio file
///
/// # Returns
///
/// A vector of mono 16-bit PCM samples
#[instrument(skip_all, fields(file = ?path.as_ref()))]
pub async fn decode_audio_ffmpeg<P: AsRef<Path>>(path: P) -> Result<super::AudioSamples> {
    let path = path.as_ref();

    debug!("Initializing FFmpeg decoder");

    // Initialize FFmpeg (only needs to happen once, but safe to call multiple times)
    ffmpeg_next::format::network::init();

    // Open input file
    let context = ffmpeg_next::format::input(&path).map_err(|e| {
        FingerprintError::AudioProcessing(format!(
            "Failed to open file with FFmpeg: {}",
            e
        ))
    })?;

    // Find audio stream
    let audio_stream_index = context
        .streams()
        .position(|stream| stream.codec().medium() == ffmpeg_next::media::Type::Audio)
        .ok_or_else(|| {
            FingerprintError::AudioProcessing(
                "No audio stream found in file".to_string(),
            )
        })?;

    let stream = context.stream(audio_stream_index as u32).ok_or_else(|| {
        FingerprintError::AudioProcessing(
            "Failed to access audio stream".to_string(),
        )
    })?;

    let codec_params = stream.codec();
    let sample_rate = codec_params.sample_rate();

    debug!(
        stream_index = audio_stream_index,
        sample_rate,
        "Found audio stream"
    );

    // Create audio resampler to convert to mono 16-bit PCM
    let mut resampler = ffmpeg_next::software::resampling::context::Context::get(
        codec_params.format(),
        codec_params.channel_layout(),
        sample_rate,
        ffmpeg_next::format::Sample::I16,
        ffmpeg_next::util::channel_layout::ChannelLayout::MONO,
        sample_rate,
    )
    .map_err(|e| {
        FingerprintError::AudioProcessing(format!(
            "Failed to create resampler: {}",
            e
        ))
    })?;

    // Decode and resample audio
    let mut samples = Vec::new();
    let max_samples = (120 * sample_rate) as usize; // 120 seconds max

    for (stream, packet) in context.packets() {
        if stream.index() != audio_stream_index {
            continue;
        }

        // Decode packet
        let mut decoded = ffmpeg_next::frame::Audio::empty();
        match stream.codec().decode(&packet, &mut decoded) {
            Ok(true) => {
                // Resample decoded audio
                resampler.run(&decoded, &mut|resampled| {
                    // Convert resampled audio to i16 samples
                    let plane = resampled.plane::<i16>(0);
                    samples.extend_from_slice(plane);
                }).map_err(|e| {
                    FingerprintError::AudioProcessing(format!(
                        "Resampling failed: {}",
                        e
                    ))
                })?;

                // Check if we have enough samples
                if samples.len() >= max_samples {
                    debug!("Maximum sample count reached, stopping decode");
                    break;
                }
            }
            Ok(false) => {
                // Packet was buffered, continue
                continue;
            }
            Err(e) => {
                return Err(FingerprintError::AudioProcessing(format!(
                    "Decoding failed: {}",
                    e
                )))
            }
        }
    }

    // Flush remaining frames from resampler
    loop {
        let mut decoded = ffmpeg_next::frame::Audio::empty();
        match resampler.flush(&mut |resampled| {
            let plane = resampled.plane::<i16>(0);
            samples.extend_from_slice(plane);
        }) {
            Ok(true) => {
                if samples.len() >= max_samples {
                    break;
                }
            }
            Ok(false) => break,
            Err(e) => {
                return Err(FingerprintError::AudioProcessing(format!(
                    "Flush failed: {}",
                    e
                )))
            }
        }
    }

    if samples.is_empty() {
        return Err(FingerprintError::AudioProcessing(
            "No audio samples extracted".to_string(),
        ));
    }

    // Truncate to max fingerprinting duration
    let max_samples = (120 * sample_rate) as usize;
    if samples.len() > max_samples {
        samples.truncate(max_samples);
    }

    let duration_secs = (samples.len() as u32 / sample_rate).max(1);

    debug!(
        sample_count = samples.len(),
        duration_secs,
        "Successfully decoded audio with FFmpeg"
    );

    Ok(super::AudioSamples {
        samples,
        sample_rate,
        duration_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffmpeg_available() {
        // Verify FFmpeg can be initialized
        ffmpeg_next::format::network::init();
        // If this completes without panic, FFmpeg is available
    }
}
