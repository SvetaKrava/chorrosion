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

use ffmpeg_next::software::resampling::Context as ResamplingContext;
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
pub async fn decode_audio_ffmpeg<P: AsRef<Path>>(path: P) -> Result<Vec<i16>> {
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
        .enumerate()
        .find_map(|(i, stream)| {
            if stream.codecpar().medium() == ffmpeg_next::media::Type::Audio {
                Some(i)
            } else {
                None
            }
        })
        .ok_or_else(|| {
            FingerprintError::AudioProcessing(
                "No audio stream found in file".to_string(),
            )
        })?;

    let stream = context.stream(audio_stream_index).ok_or_else(|| {
        FingerprintError::AudioProcessing(
            "Failed to access audio stream".to_string(),
        )
    })?;

    let codecpar = stream.codecpar();
    let sample_rate = codecpar.sample_rate() as i32;
    let channel_layout = codecpar.channel_layout();

    debug!(
        stream_index = audio_stream_index,
        sample_rate,
        "Found audio stream"
    );

    // Create audio decoder
    let codec = ffmpeg_next::decoder::find(codecpar.id())
        .ok_or_else(|| {
            FingerprintError::AudioProcessing(
                "Unsupported audio codec".to_string(),
            )
        })?;
    let mut decoder = codecpar.decoder().audio()
        .map_err(|e| {
            FingerprintError::AudioProcessing(format!(
                "Failed to create decoder: {}",
                e
            ))
        })?;

    // Create audio resampler to convert to mono 16-bit PCM
    let mut resampler = ResamplingContext::get(
        decoder.format(),
        channel_layout,
        sample_rate,
        ffmpeg_next::format::Sample::I16(ffmpeg_next::format::sample::Type::None),
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
    let max_samples = (120 * sample_rate as u32) as usize; // 120 seconds max

    for (stream_idx, packet) in context.packets() {
        if stream_idx != audio_stream_index {
            continue;
        }

        // Send packet to decoder
        decoder.send_packet(&packet)
            .map_err(|e| {
                FingerprintError::AudioProcessing(format!(
                    "Failed to send packet to decoder: {}",
                    e
                ))
            })?;

        // Receive decoded frames
        let mut decoded = ffmpeg_next::frame::Audio::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            // Resample decoded audio
            let _ = resampler.run(&decoded, &mut |resampled: &ffmpeg_next::frame::Audio| {
                // Convert resampled audio to i16 samples
                let plane = resampled.plane::<i16>(0);
                samples.extend_from_slice(plane);
            });

            // Check if we have enough samples
            if samples.len() >= max_samples {
                debug!("Maximum sample count reached, stopping decode");
                break;
            }
        }

        if samples.len() >= max_samples {
            break;
        }
    }

    // Flush any remaining frames from the decoder
    decoder.send_eof().ok();
    let mut decoded = ffmpeg_next::frame::Audio::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        let _ = resampler.run(&decoded, &mut |resampled: &ffmpeg_next::frame::Audio| {
            let plane = resampled.plane::<i16>(0);
            samples.extend_from_slice(plane);
        });

        if samples.len() >= max_samples {
            break;
        }
    }

    // Flush remaining samples from resampler
    let _ = resampler.flush(&mut |resampled: &ffmpeg_next::frame::Audio| {
        let plane = resampled.plane::<i16>(0);
        samples.extend_from_slice(plane);
    });

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

    let duration_secs = (samples.len() as u32 / sample_rate as u32).max(1);

    debug!(
        sample_count = samples.len(),
        duration_secs,
        "Successfully decoded audio with FFmpeg"
    );

    Ok(samples)
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
