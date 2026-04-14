// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared audio byte fixtures for unit tests across the application crate.
//!
//! Centralised here to prevent drift when the fixtures need to be adjusted
//! (e.g. when a lofty version update changes what a "valid" file must look
//! like).  All test modules that need real audio bytes should import from
//! this module instead of duplicating the constants.

/// Minimal valid MPEG/MP3 file: ID3v2.4 header followed by two identical
/// MPEG1 Layer-3 frames at 32 kbps / 44100 Hz / Joint Stereo (104 bytes
/// each).  Two frames are required so that lofty's `cmp_header` cross-check
/// succeeds and the file type is positively identified as MPEG during the
/// write path.
///
/// Frame header bytes `[0xFF, 0xFB, 0x10, 0x44]`:
///   sync=0xFFE, MPEG1, Layer3, 32 kbps, 44100 Hz, no padding, Joint Stereo
///   frame_length = floor(1152 × 32000 / (8 × 44100)) = 104 bytes
///
/// Layout: [0..10) ID3v2 header | [10..114) frame-1 | [114..218) frame-2
pub(crate) const MINIMAL_MP3: &[u8] = &{
    const FRAME_HDR: [u8; 4] = [0xFF, 0xFB, 0x10, 0x44];
    let mut b = [0u8; 218];
    // ID3v2.4 header at offset 0 (10 bytes, empty tag – size field = 0)
    b[0] = b'I';
    b[1] = b'D';
    b[2] = b'3';
    b[3] = 4; // version: ID3v2.4
              // Frame 1 header at offset 10 (frame_length = 104 bytes)
    b[10] = FRAME_HDR[0];
    b[11] = FRAME_HDR[1];
    b[12] = FRAME_HDR[2];
    b[13] = FRAME_HDR[3];
    // Frame 2 header at offset 10 + 104 = 114
    b[114] = FRAME_HDR[0];
    b[115] = FRAME_HDR[1];
    b[116] = FRAME_HDR[2];
    b[117] = FRAME_HDR[3];
    b
};

/// Minimal valid FLAC stream: 4-byte stream marker + STREAMINFO metadata
/// block (NOT the last-block, 34 bytes of well-formed but silent data) +
/// an empty PADDING block (last-block flag set, size=0).
///
/// The PADDING block is required to prevent an index-out-of-bounds panic in
/// lofty's FLAC writer when it tries to add padding to a file whose only
/// existing block is STREAMINFO.
pub(crate) const MINIMAL_FLAC: &[u8] = &[
    b'f', b'L', b'a', b'C', // stream marker
    0x00, 0x00, 0x00, 0x22, // NOT last block + STREAMINFO type 0 + size=34
    0x00, 0x10, 0x00, 0x10, // min/max block size = 16
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // min/max frame size = 0 (unknown)
    0x0A, 0xC4, 0x40, 0xF0, 0x00, 0x00, 0x00, 0x00, // 44100 Hz, 1ch, 16-bit, 0 samples
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // MD5 signature (bytes 1–8)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // MD5 signature (bytes 9–16)
    0x81, 0x00, 0x00, 0x00, // last block + PADDING type 1 + size=0
];
