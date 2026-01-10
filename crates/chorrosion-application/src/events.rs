// SPDX-License-Identifier: GPL-3.0-or-later
use std::sync::{Arc, Mutex};

use chorrosion_domain::DomainEvent;
use serde::Serialize;
use serde_json::json;

/// Event publisher abstraction
pub trait EventPublisher: Send + Sync {
    fn publish<T>(&self, event: &DomainEvent<T>)
    where
        T: Serialize + Send + Sync + 'static;
}

/// A minimal in-memory event bus that stores serialized events.
#[derive(Clone, Default)]
pub struct InMemoryEventBus {
    inner: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("Failed to acquire lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Retrieve and clear all captured events
    pub fn drain(&self) -> Vec<serde_json::Value> {
        let mut guard = self.inner.lock().expect("Failed to acquire lock");
        std::mem::take(&mut *guard)
    }
}

impl EventPublisher for InMemoryEventBus {
    fn publish<T>(&self, event: &DomainEvent<T>)
    where
        T: Serialize + Send + Sync + 'static,
    {
        let value = json!({
            "name": event.name,
            "occurred_at": event.occurred_at,
            "payload": event.payload,
        });
        self.inner
            .lock()
            .expect("Failed to acquire lock")
            .push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chorrosion_domain::{TrackFileId, TrackFileImported, TrackFileImportedPayload, TrackId};

    #[test]
    fn publish_and_drain_events() {
        let bus = InMemoryEventBus::new();
        assert!(bus.is_empty());

        let payload = TrackFileImportedPayload {
            track_id: TrackId::new(),
            track_file_id: TrackFileId::new(),
            path: "music/Artist/Album/01 - Song.flac".to_string(),
        };
        let evt: TrackFileImported = DomainEvent::new("track.file.imported", payload);

        bus.publish(&evt);
        assert_eq!(bus.len(), 1);

        let drained = bus.drain();
        assert_eq!(drained.len(), 1);
        let v = &drained[0];
        assert_eq!(v["name"], "track.file.imported");
        assert!(v["payload"]["path"]
            .as_str()
            .expect("Failed to get path")
            .ends_with("Song.flac"));
        assert!(bus.is_empty());
    }
}
