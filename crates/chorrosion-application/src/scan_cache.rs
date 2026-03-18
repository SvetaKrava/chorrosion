// SPDX-License-Identifier: GPL-3.0-or-later

//! In-memory cache for directory scan results.
//!
//! Scanning a large music library directory tree is an expensive filesystem operation.
//! [`DirScanCache`] lets callers avoid re-walking the same directories within a short
//! window by caching the [`ScannedAudioFile`] list keyed on the root path.
//!
//! [`cached_scan_audio_files`] is a drop-in wrapper around the bare [`scan_audio_files`]
//! function that checks and populates the cache automatically.

use crate::import_matching::{scan_audio_files, ImportMatchingError, ScannedAudioFile};
use moka::sync::Cache;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// TTL for directory scan results: 5 minutes.
const SCAN_CACHE_TTL: Duration = Duration::from_secs(300);
/// Maximum number of root-path entries kept in memory.
const SCAN_CACHE_MAX: u64 = 1_000;

/// Bounded, TTL-evicting cache for [`ScannedAudioFile`] vectors.
///
/// Values are wrapped in [`Arc`] so that multiple concurrent callers share the same
/// allocation rather than copying the full vector.
///
/// Cloning a `DirScanCache` is cheap: both instances share the same backing store.
#[derive(Clone, Debug)]
pub struct DirScanCache {
    inner: Cache<PathBuf, Arc<Vec<ScannedAudioFile>>>,
}

impl DirScanCache {
    /// Create a new `DirScanCache` with the default capacity and 5-minute TTL.
    pub fn new() -> Self {
        Self {
            inner: Cache::builder()
                .max_capacity(SCAN_CACHE_MAX)
                .time_to_live(SCAN_CACHE_TTL)
                .build(),
        }
    }

    /// Look up a previously cached scan for `path`.  Returns `None` on a miss or
    /// after the TTL has elapsed.
    pub fn get(&self, path: &Path) -> Option<Arc<Vec<ScannedAudioFile>>> {
        self.inner.get(path)
    }

    /// Store scan results for `path`.
    pub fn insert(&self, path: PathBuf, files: Arc<Vec<ScannedAudioFile>>) {
        self.inner.insert(path, files);
    }

    /// Remove a cache entry.  No-op if absent.
    pub fn invalidate(&self, path: &Path) {
        self.inner.invalidate(path);
    }
}

impl Default for DirScanCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Scan `root` for audio files, returning results from the cache when available.
///
/// On a cache miss the directory tree is walked normally and the results are stored
/// for future calls within the TTL window.  Pass a `&DirScanCache` obtained from
/// [`AppState`][crate::AppState] so all callers share the same cache.
pub fn cached_scan_audio_files(
    root: impl AsRef<Path>,
    cache: &DirScanCache,
) -> Result<Arc<Vec<ScannedAudioFile>>, ImportMatchingError> {
    let root = root.as_ref();
    let key = root.to_path_buf();

    if let Some(cached) = cache.get(&key) {
        debug!(target: "cache", path = %root.display(), "directory scan cache HIT");
        return Ok(cached);
    }

    debug!(target: "cache", path = %root.display(), "directory scan cache MISS");
    let files = scan_audio_files(root)?;
    let arc = Arc::new(files);
    cache.insert(key, arc.clone());
    Ok(arc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_temp_dir_with_mp3() -> TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("track.mp3"), b"fake mp3").expect("write");
        dir
    }

    #[test]
    fn miss_then_hit_returns_same_results() {
        let dir = make_temp_dir_with_mp3();
        let cache = DirScanCache::new();

        let first = cached_scan_audio_files(dir.path(), &cache).expect("first scan ok");
        let second = cached_scan_audio_files(dir.path(), &cache).expect("second scan ok");

        assert_eq!(first.len(), second.len());
        // Both calls return the same Arc — pointer equality confirms cache hit.
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn invalidate_causes_fresh_scan() {
        let dir = make_temp_dir_with_mp3();
        let cache = DirScanCache::new();

        let first = cached_scan_audio_files(dir.path(), &cache).expect("first ok");
        cache.invalidate(dir.path());
        let second = cached_scan_audio_files(dir.path(), &cache).expect("second ok");

        // After invalidation a fresh scan is performed — different Arc, same content.
        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(first.len(), second.len());
    }
}
