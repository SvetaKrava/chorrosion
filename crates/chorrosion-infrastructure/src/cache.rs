// SPDX-License-Identifier: GPL-3.0-or-later

//! In-memory TTL caches for hot data paths.
//!
//! [`ResponseCache`] stores pre-serialized API response bodies keyed by request URI so that
//! repeated identical GET requests bypass the database entirely.

use bytes::Bytes;
use moka::sync::Cache;
use std::time::Duration;

/// Bounded, TTL-evicting cache for serialized API response bodies.
///
/// Keyed by the request URI (path + query string).  Values are the raw JSON bytes that
/// would be written to the response body.  Uses a `moka` sync cache so concurrent reads
/// never block.
///
/// Created with a configurable maximum capacity and TTL.  Passing `ttl_seconds = 0` uses
/// a 1-second minimum TTL (entries expire almost immediately, effectively disabling the
/// cache without requiring a separate code path).
#[derive(Clone, Debug)]
pub struct ResponseCache {
    inner: Cache<String, Bytes>,
    ttl_seconds: u64,
}

impl ResponseCache {
    /// Create a new `ResponseCache`.
    ///
    /// * `max_capacity` – maximum number of entries kept before eviction.
    /// * `ttl_seconds`  – how long an entry lives before it is silently dropped on the
    ///   next access.  `0` is treated as `1` second (near-instant expiry).
    pub fn new(max_capacity: u64, ttl_seconds: u64) -> Self {
        let ttl = Duration::from_secs(ttl_seconds.max(1));
        let inner = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(ttl)
            .build();
        Self { inner, ttl_seconds }
    }

    /// Returns `true` if the cache is configured with a positive TTL (i.e. is not
    /// effectively disabled by a zero TTL setting).
    pub fn is_enabled(&self) -> bool {
        self.ttl_seconds > 0
    }

    /// Look up a cached response body.  Returns `None` on a cache miss or if the entry
    /// has expired.
    pub fn get(&self, key: &str) -> Option<Bytes> {
        self.inner.get(key)
    }

    /// Store a response body.  Overwrites any existing entry for the same key.
    pub fn insert(&self, key: String, value: Bytes) {
        self.inner.insert(key, value);
    }

    /// Remove a single cache entry.  No-op if the key is absent.
    pub fn invalidate(&self, key: &str) {
        self.inner.invalidate(key);
    }

    /// Drop all cached entries.
    pub fn invalidate_all(&self) {
        self.inner.invalidate_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_none_on_miss() {
        let cache = ResponseCache::new(100, 60);
        assert!(cache.get("missing").is_none());
    }

    #[test]
    fn insert_then_get_returns_value() {
        let cache = ResponseCache::new(100, 60);
        cache.insert("key".to_string(), Bytes::from_static(b"{\"ok\":true}"));
        assert_eq!(
            cache.get("key").unwrap(),
            Bytes::from_static(b"{\"ok\":true}")
        );
    }

    #[test]
    fn invalidate_removes_entry() {
        let cache = ResponseCache::new(100, 60);
        cache.insert("key".to_string(), Bytes::from_static(b"data"));
        cache.invalidate("key");
        assert!(cache.get("key").is_none());
    }

    #[test]
    fn invalidate_all_removes_all_entries() {
        let cache = ResponseCache::new(100, 60);
        cache.insert("a".to_string(), Bytes::from_static(b"1"));
        cache.insert("b".to_string(), Bytes::from_static(b"2"));
        cache.invalidate_all();
        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_none());
    }

    #[test]
    fn zero_ttl_is_treated_as_one_second_and_enabled_is_false() {
        let cache = ResponseCache::new(100, 0);
        assert!(!cache.is_enabled());
    }

    #[test]
    fn nonzero_ttl_is_enabled() {
        let cache = ResponseCache::new(100, 60);
        assert!(cache.is_enabled());
    }
}
