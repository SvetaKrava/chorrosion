// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListEntityType {
    Artist,
    Album,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalListEntry {
    pub entity_type: ListEntityType,
    pub external_id: String,
    pub name: String,
    pub artist_name: Option<String>,
    pub source_url: Option<String>,
    pub followed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListProviderCapabilities {
    pub supports_artists: bool,
    pub supports_albums: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListProviderHealth {
    pub ok: bool,
    pub message: Option<String>,
}

#[async_trait]
pub trait ListProvider: Send + Sync {
    fn provider_name(&self) -> &'static str;

    fn capabilities(&self) -> ListProviderCapabilities;

    async fn health_check(&self) -> Result<ListProviderHealth>;

    async fn fetch_followed_artists(&self) -> Result<Vec<ExternalListEntry>>;

    async fn fetch_saved_albums(&self) -> Result<Vec<ExternalListEntry>>;
}

pub fn dedupe_list_entries(entries: Vec<ExternalListEntry>) -> Vec<ExternalListEntry> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::with_capacity(entries.len());

    for entry in entries {
        let key = (entry.entity_type, entry.external_id.clone());
        if seen.insert(key) {
            deduped.push(entry);
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupe_list_entries_removes_duplicate_external_ids() {
        let entries = vec![
            ExternalListEntry {
                entity_type: ListEntityType::Artist,
                external_id: "artist-1".to_string(),
                name: "Artist One".to_string(),
                artist_name: None,
                source_url: None,
                followed_at: None,
            },
            ExternalListEntry {
                entity_type: ListEntityType::Artist,
                external_id: "artist-1".to_string(),
                name: "Artist One Duplicate".to_string(),
                artist_name: None,
                source_url: None,
                followed_at: None,
            },
            ExternalListEntry {
                entity_type: ListEntityType::Album,
                external_id: "album-9".to_string(),
                name: "Album Nine".to_string(),
                artist_name: Some("Artist Z".to_string()),
                source_url: None,
                followed_at: None,
            },
        ];

        let deduped = dedupe_list_entries(entries);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].external_id, "artist-1");
        assert_eq!(deduped[1].external_id, "album-9");
    }

    #[test]
    fn dedupe_list_entries_keeps_different_entity_types() {
        let entries = vec![
            ExternalListEntry {
                entity_type: ListEntityType::Artist,
                external_id: "42".to_string(),
                name: "Artist 42".to_string(),
                artist_name: None,
                source_url: None,
                followed_at: None,
            },
            ExternalListEntry {
                entity_type: ListEntityType::Album,
                external_id: "42".to_string(),
                name: "Album 42".to_string(),
                artist_name: Some("Artist 42".to_string()),
                source_url: None,
                followed_at: None,
            },
        ];

        let deduped = dedupe_list_entries(entries);
        assert_eq!(deduped.len(), 2);
    }
}
