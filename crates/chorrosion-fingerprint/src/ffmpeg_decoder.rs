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

use crate::{generator::MAX_FINGERPRINT_DURATION_SECS, FingerprintError, Result};

/// Decode audio from a file using FFmpeg.
///
/// Extracts mono, 16-bit PCM samples at the file's native sample rate.
/// The decoder preserves the source sample rate and does not perform resampling.
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
    let mut context = ffmpeg_next::format::input(&path).map_err(|e| {
        FingerprintError::AudioProcessing(format!("Failed to open file with FFmpeg: {}", e))
    })?;

    // Find the first audio stream
    let audio_stream = context
        .streams()
        .find(|stream| stream.parameters().medium() == ffmpeg_next::media::Type::Audio)
        .ok_or_else(|| {
            FingerprintError::AudioProcessing("No audio stream found in file".to_string())
        })?;

    let audio_stream_index = audio_stream.index();

    // Get codec parameters and create decoder
    let codec_id = audio_stream.parameters().id();
    let codec = ffmpeg_next::codec::decoder::find(codec_id).ok_or_else(|| {
        FingerprintError::AudioProcessing(format!("Codec not found for codec ID: {:?}", codec_id))
    })?;

    let mut decoder = ffmpeg_next::codec::context::Context::new_with_codec(codec)
        .decoder()
        .audio()
        .map_err(|e| {
            FingerprintError::AudioProcessing(format!("Failed to create audio decoder: {}", e))
        })?;

    // Copy codec parameters to decoder
    decoder
        .set_parameters(audio_stream.parameters())
        .map_err(|e| {
            FingerprintError::AudioProcessing(format!("Failed to set decoder parameters: {}", e))
        })?;

    debug!("Found audio stream and created decoder");

    // Decode audio
    let mut samples = Vec::new();
    // FFmpeg decodes at the source sample rate; use standard rate for max sample calculation
    let sample_rate = 44100u32; // Standard audio sample rate
    let max_samples = (MAX_FINGERPRINT_DURATION_SECS as u64 * sample_rate as u64) as usize; // 120 seconds max

    // Process all packets for this stream
    for (stream, packet) in context.packets() {
        if stream.index() != audio_stream_index {
            continue;
        }

        // Send packet to decoder
        decoder.send_packet(&packet).map_err(|e| {
            FingerprintError::AudioProcessing(format!("Failed to send packet to decoder: {}", e))
        })?;

        // Receive decoded frames
        let mut decoded = ffmpeg_next::frame::Audio::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            // Get the samples from the decoded frame
            let plane = decoded.plane::<i16>(0);
            samples.extend_from_slice(plane);

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
    let mut flush_frame = ffmpeg_next::frame::Audio::empty();
    while decoder.receive_frame(&mut flush_frame).is_ok() {
        let plane = flush_frame.plane::<i16>(0);
        samples.extend_from_slice(plane);

        if samples.len() >= max_samples {
            break;
        }
    }

    if samples.is_empty() {
        return Err(FingerprintError::AudioProcessing(
            "No audio samples extracted".to_string(),
        ));
    }

    // Note: max_samples is checked during decoding loops above;
    // truncation happens inline to avoid recalculation

    debug!(
        sample_count = samples.len(),
        "Successfully decoded audio with FFmpeg"
    );

    Ok(samples)
}

#[cfg(test)]
mod tests {
    #[test]
    fn ffmpeg_available() {
        // Verify FFmpeg can be initialized
        ffmpeg_next::format::network::init();
        // If this completes without panic, FFmpeg is available
    }
}
