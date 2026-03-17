// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use async_trait::async_trait;
use chorrosion_config::AppConfig;
use chorrosion_musicbrainz::MusicBrainzClient;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

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

pub struct MusicBrainzListProvider {
    enabled: bool,
    client: MusicBrainzClient,
    artist_mbids: Vec<Uuid>,
    album_mbids: Vec<Uuid>,
}

impl MusicBrainzListProvider {
    pub fn from_config(config: &AppConfig) -> Result<Self> {
        let musicbrainz = &config.lists.musicbrainz;
        let base_url = musicbrainz
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let artist_mbids = parse_mbids(&musicbrainz.artist_mbids);
        let album_mbids = parse_mbids(&musicbrainz.album_mbids);

        let client = if let Some(url) = base_url {
            MusicBrainzClient::builder().base_url(url).build()?
        } else {
            MusicBrainzClient::new()?
        };

        Ok(Self {
            enabled: musicbrainz.enabled,
            client,
            artist_mbids,
            album_mbids,
        })
    }
}

fn parse_mbids(raw_mbids: &[String]) -> Vec<Uuid> {
    raw_mbids
        .iter()
        .filter_map(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            match Uuid::parse_str(trimmed) {
                Ok(uuid) => Some(uuid),
                Err(_) => {
                    tracing::warn!(
                        target: "application",
                        mbid = %trimmed,
                        "Skipping invalid MusicBrainz MBID"
                    );
                    None
                }
            }
        })
        .collect()
}

#[async_trait]
impl ListProvider for MusicBrainzListProvider {
    fn provider_name(&self) -> &'static str {
        "musicbrainz"
    }

    fn capabilities(&self) -> ListProviderCapabilities {
        ListProviderCapabilities {
            supports_artists: true,
            supports_albums: true,
        }
    }

    async fn health_check(&self) -> Result<ListProviderHealth> {
        let has_entries = !self.artist_mbids.is_empty() || !self.album_mbids.is_empty();
        Ok(ListProviderHealth {
            ok: self.enabled && has_entries,
            message: if !self.enabled {
                Some("provider disabled".to_string())
            } else if !has_entries {
                Some("no MusicBrainz MBIDs configured".to_string())
            } else {
                None
            },
        })
    }

    async fn fetch_followed_artists(&self) -> Result<Vec<ExternalListEntry>> {
        if !self.enabled {
            return Ok(vec![]);
        }

        let mut entries = Vec::with_capacity(self.artist_mbids.len());
        for mbid in &self.artist_mbids {
            match self.client.lookup_artist(*mbid).await {
                Ok(artist) => entries.push(ExternalListEntry {
                    entity_type: ListEntityType::Artist,
                    external_id: artist.id.to_string(),
                    name: artist.name,
                    artist_name: None,
                    source_url: Some(format!("https://musicbrainz.org/artist/{}", artist.id)),
                    followed_at: None,
                }),
                Err(error) => {
                    tracing::warn!(
                        target: "application",
                        mbid = %mbid,
                        ?error,
                        "Failed to import artist from MusicBrainz"
                    );
                }
            }
        }

        Ok(dedupe_list_entries(entries))
    }

    async fn fetch_saved_albums(&self) -> Result<Vec<ExternalListEntry>> {
        if !self.enabled {
            return Ok(vec![]);
        }

        let mut entries = Vec::with_capacity(self.album_mbids.len());
        for mbid in &self.album_mbids {
            match self.client.lookup_album(*mbid).await {
                Ok(album) => {
                    let artist_name = album.artist_credit.first().map(|ac| ac.name.clone());
                    entries.push(ExternalListEntry {
                        entity_type: ListEntityType::Album,
                        external_id: album.id.to_string(),
                        name: album.title,
                        artist_name,
                        source_url: Some(format!(
                            "https://musicbrainz.org/release-group/{}",
                            album.id
                        )),
                        followed_at: None,
                    });
                }
                Err(error) => {
                    tracing::warn!(
                        target: "application",
                        mbid = %mbid,
                        ?error,
                        "Failed to import album from MusicBrainz"
                    );
                }
            }
        }

        Ok(dedupe_list_entries(entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{method, path, query_param},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn dedupe_list_entries_removes_entries_with_same_entity_type_and_external_id() {
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

    #[tokio::test]
    async fn musicbrainz_provider_imports_artists_and_albums() {
        let server = MockServer::start().await;

        let artist_id = "11111111-1111-1111-1111-111111111111";
        Mock::given(method("GET"))
            .and(path(format!("/artist/{artist_id}")))
            .and(query_param("fmt", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": artist_id,
                "name": "Artist One",
                "sort-name": "One, Artist",
                "type": "Group",
                "country": "US"
            })))
            .mount(&server)
            .await;

        let album_id = "22222222-2222-2222-2222-222222222222";
        Mock::given(method("GET"))
            .and(path(format!("/release-group/{album_id}")))
            .and(query_param("fmt", "json"))
            .and(query_param("inc", "artist-credits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": album_id,
                "title": "Album Two",
                "primary-type": "Album",
                "secondary-types": [],
                "first-release-date": "2020-01-01",
                "artist-credit": [{
                    "name": "Artist One",
                    "artist": {
                        "id": artist_id,
                        "name": "Artist One",
                        "sort-name": "One, Artist"
                    }
                }]
            })))
            .mount(&server)
            .await;

        let config = AppConfig {
            lists: chorrosion_config::ListsConfig {
                musicbrainz: chorrosion_config::MusicBrainzListsConfig {
                    enabled: true,
                    base_url: Some(server.uri()),
                    artist_mbids: vec![artist_id.to_string()],
                    album_mbids: vec![album_id.to_string()],
                },
            },
            ..AppConfig::default()
        };

        let provider = MusicBrainzListProvider::from_config(&config).unwrap();
        let artists = provider.fetch_followed_artists().await.unwrap();
        let albums = provider.fetch_saved_albums().await.unwrap();

        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].external_id, artist_id);
        assert_eq!(artists[0].name, "Artist One");

        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].external_id, album_id);
        assert_eq!(albums[0].name, "Album Two");
        assert_eq!(albums[0].artist_name.as_deref(), Some("Artist One"));
    }

    #[tokio::test]
    async fn musicbrainz_provider_health_check_reflects_config() {
        let config = AppConfig {
            lists: chorrosion_config::ListsConfig {
                musicbrainz: chorrosion_config::MusicBrainzListsConfig {
                    enabled: true,
                    base_url: None,
                    artist_mbids: vec![],
                    album_mbids: vec![],
                },
            },
            ..AppConfig::default()
        };

        let provider = MusicBrainzListProvider::from_config(&config).unwrap();
        let health = provider.health_check().await.unwrap();
        assert!(!health.ok);
        assert_eq!(
            health.message.as_deref(),
            Some("no MusicBrainz MBIDs configured")
        );
    }
}
