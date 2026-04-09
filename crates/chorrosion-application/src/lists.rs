// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use async_trait::async_trait;
use chorrosion_config::AppConfig;
use chorrosion_domain::{Album, Artist};
use chorrosion_infrastructure::repositories::{AlbumRepository, ArtistRepository};
use chorrosion_metadata::lastfm::LastFmClient;
use chorrosion_musicbrainz::MusicBrainzClient;
use chrono::{DateTime, Utc};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Encoding set for URL path segments: encodes all non-alphanumeric characters
/// except the RFC 3986 unreserved characters (`-`, `_`, `.`, `~`). This ensures
/// spaces and special characters like `/` are encoded while keeping common
/// artist/album name punctuation readable.
const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListAutoAddSummary {
    pub artists_created: usize,
    pub artists_updated: usize,
    pub artists_skipped: usize,
    pub albums_created: usize,
    pub albums_updated: usize,
    pub albums_skipped: usize,
    pub albums_skipped_missing_artist: usize,
}

pub async fn auto_add_from_list_entries<AR, ALR>(
    artist_repo: &AR,
    album_repo: &ALR,
    artist_entries: Vec<ExternalListEntry>,
    album_entries: Vec<ExternalListEntry>,
) -> Result<ListAutoAddSummary>
where
    AR: ArtistRepository,
    ALR: AlbumRepository,
{
    let mut summary = ListAutoAddSummary::default();

    let artist_entries = dedupe_list_entries(artist_entries)
        .into_iter()
        .filter(|entry| entry.entity_type == ListEntityType::Artist)
        .collect::<Vec<_>>();
    let album_entries = dedupe_list_entries(album_entries)
        .into_iter()
        .filter(|entry| entry.entity_type == ListEntityType::Album)
        .collect::<Vec<_>>();

    for entry in artist_entries {
        if artist_repo
            .get_by_foreign_id(&entry.external_id)
            .await?
            .is_some()
        {
            summary.artists_skipped += 1;
            continue;
        }

        if let Some(existing) = artist_repo.get_by_name(&entry.name).await? {
            if existing.foreign_artist_id.is_none() {
                let mut updated = existing;
                updated.foreign_artist_id = Some(entry.external_id.clone());
                updated.updated_at = Utc::now();
                artist_repo.update(updated).await?;
                summary.artists_updated += 1;
            } else {
                summary.artists_skipped += 1;
            }
            continue;
        }

        let mut artist = Artist::new(entry.name);
        artist.foreign_artist_id = Some(entry.external_id);
        artist_repo.create(artist).await?;
        summary.artists_created += 1;
    }

    for entry in album_entries {
        let Some(artist_name) = entry.artist_name.as_deref() else {
            summary.albums_skipped_missing_artist += 1;
            continue;
        };

        let artist = if let Some(existing_artist) = artist_repo.get_by_name(artist_name).await? {
            existing_artist
        } else {
            let created = Artist::new(artist_name.to_string());
            let created = artist_repo.create(created).await?;
            summary.artists_created += 1;
            created
        };

        if album_repo
            .get_by_foreign_id(&entry.external_id)
            .await?
            .is_some()
        {
            summary.albums_skipped += 1;
            continue;
        }

        if let Some(existing_album) = album_repo
            .get_by_artist_and_title(artist.id, &entry.name)
            .await?
        {
            if existing_album.foreign_album_id.is_none() {
                let mut updated = existing_album;
                updated.foreign_album_id = Some(entry.external_id.clone());
                updated.updated_at = Utc::now();
                album_repo.update(updated).await?;
                summary.albums_updated += 1;
            } else {
                summary.albums_skipped += 1;
            }
            continue;
        }

        let mut album = Album::new(artist.id, entry.name);
        album.foreign_album_id = Some(entry.external_id);
        album_repo.create(album).await?;
        summary.albums_created += 1;
    }

    Ok(summary)
}

pub struct MusicBrainzListProvider {
    enabled: bool,
    client: MusicBrainzClient,
    artist_mbids: Vec<Uuid>,
    album_mbids: Vec<Uuid>,
}

pub struct SpotifyPlaylistListProvider {
    enabled: bool,
    http_client: reqwest::Client,
    base_url: String,
    access_token: Option<String>,
    playlist_ids: Vec<String>,
    market: Option<String>,
}

fn build_spotify_http_client() -> Client {
    crate::http_client::build_http_client()
}

impl SpotifyPlaylistListProvider {
    pub fn from_config(config: &AppConfig) -> Self {
        let spotify = &config.lists.spotify;
        let base_url = spotify
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("https://api.spotify.com/v1")
            .trim_end_matches('/')
            .to_string();

        let access_token = spotify
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let playlist_ids = spotify
            .playlist_ids
            .iter()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .collect();

        let market = spotify
            .market
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        Self {
            enabled: spotify.enabled,
            http_client: build_spotify_http_client(),
            base_url,
            access_token,
            playlist_ids,
            market,
        }
    }

    fn is_ready(&self) -> bool {
        self.enabled && self.access_token.is_some() && !self.playlist_ids.is_empty()
    }

    async fn fetch_playlist_tracks(
        &self,
        playlist_id: &str,
    ) -> Result<Vec<SpotifyPlaylistTrackItem>> {
        let token = match &self.access_token {
            Some(token) => token,
            None => return Ok(vec![]),
        };

        let mut all_items = Vec::new();
        let mut offset: usize = 0;

        loop {
            let url = format!("{}/playlists/{}/tracks", self.base_url, playlist_id);
            let mut request = self
                .http_client
                .get(url)
                .bearer_auth(token)
                .query(&[("limit", "100"), ("offset", &offset.to_string())]);

            if let Some(market) = &self.market {
                request = request.query(&[("market", market)]);
            }

            let response = request.send().await?.error_for_status()?;
            let payload: SpotifyPlaylistTracksResponse = response.json().await?;
            let count = payload.items.len();
            all_items.extend(payload.items);

            if payload.next.is_none() || count == 0 {
                break;
            }

            offset += count;
        }

        Ok(all_items)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SpotifyPlaylistTracksResponse {
    items: Vec<SpotifyPlaylistTrackItem>,
    next: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SpotifyPlaylistTrackItem {
    track: Option<SpotifyTrack>,
}

#[derive(Debug, Clone, Deserialize)]
struct SpotifyTrack {
    artists: Vec<SpotifyArtist>,
    album: Option<SpotifyAlbum>,
}

#[derive(Debug, Clone, Deserialize)]
struct SpotifyArtist {
    id: Option<String>,
    name: String,
    external_urls: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
struct SpotifyAlbum {
    id: Option<String>,
    name: String,
    artists: Vec<SpotifyArtist>,
    external_urls: Option<HashMap<String, String>>,
}

#[async_trait]
impl ListProvider for SpotifyPlaylistListProvider {
    fn provider_name(&self) -> &'static str {
        "spotify"
    }

    fn capabilities(&self) -> ListProviderCapabilities {
        ListProviderCapabilities {
            supports_artists: true,
            supports_albums: true,
        }
    }

    async fn health_check(&self) -> Result<ListProviderHealth> {
        Ok(ListProviderHealth {
            ok: self.is_ready(),
            message: if !self.enabled {
                Some("provider disabled".to_string())
            } else if self.access_token.is_none() {
                Some("Spotify access token not configured".to_string())
            } else if self.playlist_ids.is_empty() {
                Some("no Spotify playlist IDs configured".to_string())
            } else {
                None
            },
        })
    }

    async fn fetch_followed_artists(&self) -> Result<Vec<ExternalListEntry>> {
        if !self.is_ready() {
            return Ok(vec![]);
        }

        let mut entries = Vec::new();
        for playlist_id in &self.playlist_ids {
            let playlist_tracks = self.fetch_playlist_tracks(playlist_id).await;
            match playlist_tracks {
                Ok(items) => {
                    for item in items {
                        let Some(track) = item.track else {
                            continue;
                        };
                        for artist in track.artists {
                            let external_id = artist.id.unwrap_or_else(|| {
                                format!("spotify:artist:name:{}", artist.name.to_lowercase())
                            });
                            let source_url = artist
                                .external_urls
                                .as_ref()
                                .and_then(|urls| urls.get("spotify"))
                                .cloned();
                            entries.push(ExternalListEntry {
                                entity_type: ListEntityType::Artist,
                                external_id,
                                name: artist.name,
                                artist_name: None,
                                source_url,
                                followed_at: None,
                            });
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        target: "application",
                        playlist_id = %playlist_id,
                        ?error,
                        "Failed to import artists from Spotify playlist"
                    );
                }
            }
        }

        Ok(dedupe_list_entries(entries))
    }

    async fn fetch_saved_albums(&self) -> Result<Vec<ExternalListEntry>> {
        if !self.is_ready() {
            return Ok(vec![]);
        }

        let mut entries = Vec::new();
        for playlist_id in &self.playlist_ids {
            let playlist_tracks = self.fetch_playlist_tracks(playlist_id).await;
            match playlist_tracks {
                Ok(items) => {
                    for item in items {
                        let Some(track) = item.track else {
                            continue;
                        };
                        let Some(album) = track.album else {
                            continue;
                        };

                        let artist_name = album.artists.first().map(|artist| artist.name.clone());
                        let external_id = album.id.unwrap_or_else(|| {
                            format!(
                                "spotify:album:{}:{}",
                                album.name.to_lowercase(),
                                artist_name.as_deref().unwrap_or("unknown").to_lowercase()
                            )
                        });
                        let source_url = album
                            .external_urls
                            .as_ref()
                            .and_then(|urls| urls.get("spotify"))
                            .cloned();

                        entries.push(ExternalListEntry {
                            entity_type: ListEntityType::Album,
                            external_id,
                            name: album.name,
                            artist_name,
                            source_url,
                            followed_at: None,
                        });
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        target: "application",
                        playlist_id = %playlist_id,
                        ?error,
                        "Failed to import albums from Spotify playlist"
                    );
                }
            }
        }

        Ok(dedupe_list_entries(entries))
    }
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
            let artist_result = self.client.lookup_artist(*mbid).await;
            match artist_result {
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
            let album_result = self.client.lookup_album(*mbid).await;
            match album_result {
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

pub struct LastFmListProvider {
    enabled: bool,
    client: Option<LastFmClient>,
    artist_names: Vec<String>,
    album_seeds: Vec<(String, String)>,
}

impl LastFmListProvider {
    pub fn from_config(config: &AppConfig) -> Self {
        let lfm = &config.lists.lastfm;
        let api_key = lfm
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let base_url = lfm
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let client = if lfm.enabled {
            api_key
                .as_ref()
                .map(|key| LastFmClient::new(key.clone(), base_url))
        } else {
            None
        };
        Self {
            enabled: lfm.enabled,
            client,
            artist_names: lfm
                .artist_names
                .iter()
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .collect(),
            album_seeds: lfm
                .album_seeds
                .iter()
                .map(|s| (s.artist.trim().to_string(), s.album.trim().to_string()))
                .filter(|(artist, album)| !artist.is_empty() && !album.is_empty())
                .collect(),
        }
    }

    fn has_entries(&self) -> bool {
        !self.artist_names.is_empty() || !self.album_seeds.is_empty()
    }

    fn is_ready(&self) -> bool {
        self.client.is_some() && self.has_entries()
    }
}

#[async_trait]
impl ListProvider for LastFmListProvider {
    fn provider_name(&self) -> &'static str {
        "lastfm"
    }

    fn capabilities(&self) -> ListProviderCapabilities {
        ListProviderCapabilities {
            supports_artists: true,
            supports_albums: true,
        }
    }

    async fn health_check(&self) -> Result<ListProviderHealth> {
        Ok(ListProviderHealth {
            ok: self.is_ready(),
            message: if !self.enabled {
                Some("provider disabled".to_string())
            } else if self.client.is_none() {
                Some("Last.fm API key not configured".to_string())
            } else if !self.has_entries() {
                Some("no Last.fm artist names or album seeds configured".to_string())
            } else {
                None
            },
        })
    }

    async fn fetch_followed_artists(&self) -> Result<Vec<ExternalListEntry>> {
        if !self.is_ready() {
            return Ok(vec![]);
        }
        let client = self.client.as_ref().unwrap();
        let mut entries = Vec::with_capacity(self.artist_names.len());
        for artist in &self.artist_names {
            let artist_metadata = client.fetch_artist_metadata(artist).await;
            match artist_metadata {
                Ok(meta) => {
                    entries.push(ExternalListEntry {
                        entity_type: ListEntityType::Artist,
                        external_id: meta.name.to_lowercase(),
                        name: meta.name.clone(),
                        artist_name: None,
                        source_url: Some(format!(
                            "https://www.last.fm/music/{}",
                            utf8_percent_encode(&meta.name, PATH_SEGMENT)
                        )),
                        followed_at: None,
                    });
                }
                Err(error) => {
                    tracing::warn!(
                        target: "application",
                        artist = %artist,
                        ?error,
                        "Failed to import artist from Last.fm"
                    );
                }
            }
        }
        Ok(dedupe_list_entries(entries))
    }

    async fn fetch_saved_albums(&self) -> Result<Vec<ExternalListEntry>> {
        if !self.is_ready() {
            return Ok(vec![]);
        }
        let client = self.client.as_ref().unwrap();
        let mut entries = Vec::with_capacity(self.album_seeds.len());
        for (artist, album) in &self.album_seeds {
            let album_metadata = client.fetch_album_metadata(artist, album).await;
            match album_metadata {
                Ok(meta) => {
                    entries.push(ExternalListEntry {
                        entity_type: ListEntityType::Album,
                        external_id: format!(
                            "{}::{}",
                            meta.artist.to_lowercase(),
                            meta.title.to_lowercase()
                        ),
                        name: meta.title.clone(),
                        artist_name: Some(meta.artist.clone()),
                        source_url: Some(format!(
                            "https://www.last.fm/music/{}/{}",
                            utf8_percent_encode(&meta.artist, PATH_SEGMENT),
                            utf8_percent_encode(&meta.title, PATH_SEGMENT)
                        )),
                        followed_at: None,
                    });
                }
                Err(error) => {
                    tracing::warn!(
                        target: "application",
                        artist = %artist,
                        album = %album,
                        ?error,
                        "Failed to import album from Last.fm"
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
    use chorrosion_domain::{AlbumStatus, ArtistStatus};
    use chorrosion_infrastructure::repositories::{AlbumRepository, ArtistRepository, Repository};
    use std::sync::{Arc, Mutex};
    use wiremock::{
        matchers::{method, path, query_param},
        Mock, MockServer, ResponseTemplate,
    };

    #[derive(Clone, Default)]
    struct InMemoryArtistRepo {
        artists: Arc<Mutex<Vec<Artist>>>,
    }

    #[async_trait::async_trait]
    impl Repository<Artist> for InMemoryArtistRepo {
        async fn create(&self, entity: Artist) -> Result<Artist> {
            self.artists.lock().unwrap().push(entity.clone());
            Ok(entity)
        }

        async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<Artist>> {
            let id = id.into();
            Ok(self
                .artists
                .lock()
                .unwrap()
                .iter()
                .find(|artist| artist.id.to_string() == id)
                .cloned())
        }

        async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Artist>> {
            let artists = self.artists.lock().unwrap();
            Ok(artists
                .iter()
                .skip(offset.max(0) as usize)
                .take(limit.max(0) as usize)
                .cloned()
                .collect())
        }

        async fn update(&self, entity: Artist) -> Result<Artist> {
            let mut artists = self.artists.lock().unwrap();
            if let Some(existing) = artists.iter_mut().find(|artist| artist.id == entity.id) {
                *existing = entity.clone();
            }
            Ok(entity)
        }

        async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
            let id = id.into();
            let mut artists = self.artists.lock().unwrap();
            artists.retain(|artist| artist.id.to_string() != id);
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl ArtistRepository for InMemoryArtistRepo {
        async fn get_by_name(&self, name: &str) -> Result<Option<Artist>> {
            Ok(self
                .artists
                .lock()
                .unwrap()
                .iter()
                .find(|artist| artist.name.eq_ignore_ascii_case(name))
                .cloned())
        }

        async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Artist>> {
            Ok(self
                .artists
                .lock()
                .unwrap()
                .iter()
                .find(|artist| artist.foreign_artist_id.as_deref() == Some(foreign_id))
                .cloned())
        }

        async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Artist>> {
            let artists = self.artists.lock().unwrap();
            Ok(artists
                .iter()
                .filter(|artist| artist.monitored)
                .skip(offset.max(0) as usize)
                .take(limit.max(0) as usize)
                .cloned()
                .collect())
        }

        async fn get_by_status(
            &self,
            status: ArtistStatus,
            limit: i64,
            offset: i64,
        ) -> Result<Vec<Artist>> {
            let artists = self.artists.lock().unwrap();
            Ok(artists
                .iter()
                .filter(|artist| artist.status == status)
                .skip(offset.max(0) as usize)
                .take(limit.max(0) as usize)
                .cloned()
                .collect())
        }
    }

    #[derive(Clone, Default)]
    struct InMemoryAlbumRepo {
        albums: Arc<Mutex<Vec<Album>>>,
    }

    #[async_trait::async_trait]
    impl Repository<Album> for InMemoryAlbumRepo {
        async fn create(&self, entity: Album) -> Result<Album> {
            self.albums.lock().unwrap().push(entity.clone());
            Ok(entity)
        }

        async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<Album>> {
            let id = id.into();
            Ok(self
                .albums
                .lock()
                .unwrap()
                .iter()
                .find(|album| album.id.to_string() == id)
                .cloned())
        }

        async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
            let albums = self.albums.lock().unwrap();
            Ok(albums
                .iter()
                .skip(offset.max(0) as usize)
                .take(limit.max(0) as usize)
                .cloned()
                .collect())
        }

        async fn update(&self, entity: Album) -> Result<Album> {
            let mut albums = self.albums.lock().unwrap();
            if let Some(existing) = albums.iter_mut().find(|album| album.id == entity.id) {
                *existing = entity.clone();
            }
            Ok(entity)
        }

        async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
            let id = id.into();
            let mut albums = self.albums.lock().unwrap();
            albums.retain(|album| album.id.to_string() != id);
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl AlbumRepository for InMemoryAlbumRepo {
        async fn get_by_artist(
            &self,
            artist_id: chorrosion_domain::ArtistId,
            limit: i64,
            offset: i64,
        ) -> Result<Vec<Album>> {
            let albums = self.albums.lock().unwrap();
            Ok(albums
                .iter()
                .filter(|album| album.artist_id == artist_id)
                .skip(offset.max(0) as usize)
                .take(limit.max(0) as usize)
                .cloned()
                .collect())
        }

        async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Album>> {
            Ok(self
                .albums
                .lock()
                .unwrap()
                .iter()
                .find(|album| album.foreign_album_id.as_deref() == Some(foreign_id))
                .cloned())
        }

        async fn get_by_artist_and_title(
            &self,
            artist_id: chorrosion_domain::ArtistId,
            title: &str,
        ) -> Result<Option<Album>> {
            Ok(self
                .albums
                .lock()
                .unwrap()
                .iter()
                .find(|album| {
                    album.artist_id == artist_id && album.title.eq_ignore_ascii_case(title)
                })
                .cloned())
        }

        async fn get_by_status(
            &self,
            status: AlbumStatus,
            limit: i64,
            offset: i64,
        ) -> Result<Vec<Album>> {
            let albums = self.albums.lock().unwrap();
            Ok(albums
                .iter()
                .filter(|album| album.status == status)
                .skip(offset.max(0) as usize)
                .take(limit.max(0) as usize)
                .cloned()
                .collect())
        }

        async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
            let albums = self.albums.lock().unwrap();
            Ok(albums
                .iter()
                .filter(|album| album.monitored)
                .skip(offset.max(0) as usize)
                .take(limit.max(0) as usize)
                .cloned()
                .collect())
        }

        async fn get_by_album_type(
            &self,
            album_type: &str,
            limit: i64,
            offset: i64,
        ) -> Result<Vec<Album>> {
            let albums = self.albums.lock().unwrap();
            Ok(albums
                .iter()
                .filter(|album| album.album_type.as_deref() == Some(album_type))
                .skip(offset.max(0) as usize)
                .take(limit.max(0) as usize)
                .cloned()
                .collect())
        }

        async fn list_wanted_without_tracks(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
            self.get_by_status(AlbumStatus::Wanted, limit, offset).await
        }

        async fn list_cutoff_unmet_albums(&self, _limit: i64, _offset: i64) -> Result<Vec<Album>> {
            Ok(vec![])
        }

        async fn list_upcoming_releases(
            &self,
            _start: chrono::NaiveDate,
            _end: chrono::NaiveDate,
            _limit: i64,
            _offset: i64,
        ) -> Result<Vec<Album>> {
            Ok(vec![])
        }
    }

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
                spotify: chorrosion_config::SpotifyListsConfig::default(),
                lastfm: chorrosion_config::LastFmListsConfig::default(),
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
                spotify: chorrosion_config::SpotifyListsConfig::default(),
                lastfm: chorrosion_config::LastFmListsConfig::default(),
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

    #[tokio::test]
    async fn spotify_provider_imports_entries_from_playlists() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/playlists/playlist-1/tracks"))
            .and(query_param("limit", "100"))
            .and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [
                    {
                        "track": {
                            "artists": [
                                {
                                    "id": "artist-1",
                                    "name": "Artist One",
                                    "external_urls": {
                                        "spotify": "https://open.spotify.com/artist/artist-1"
                                    }
                                }
                            ],
                            "album": {
                                "id": "album-1",
                                "name": "Album One",
                                "artists": [
                                    {
                                        "id": "artist-1",
                                        "name": "Artist One",
                                        "external_urls": {
                                            "spotify": "https://open.spotify.com/artist/artist-1"
                                        }
                                    }
                                ],
                                "external_urls": {
                                    "spotify": "https://open.spotify.com/album/album-1"
                                }
                            }
                        }
                    },
                    {
                        "track": {
                            "artists": [
                                {
                                    "id": "artist-1",
                                    "name": "Artist One",
                                    "external_urls": {
                                        "spotify": "https://open.spotify.com/artist/artist-1"
                                    }
                                }
                            ],
                            "album": {
                                "id": "album-1",
                                "name": "Album One",
                                "artists": [
                                    {
                                        "id": "artist-1",
                                        "name": "Artist One"
                                    }
                                ]
                            }
                        }
                    }
                ],
                "next": null
            })))
            .mount(&server)
            .await;

        let config = AppConfig {
            lists: chorrosion_config::ListsConfig {
                musicbrainz: chorrosion_config::MusicBrainzListsConfig::default(),
                spotify: chorrosion_config::SpotifyListsConfig {
                    enabled: true,
                    base_url: Some(format!("{}/v1", server.uri())),
                    access_token: Some("test-token".to_string()),
                    playlist_ids: vec!["playlist-1".to_string()],
                    market: None,
                },
                lastfm: chorrosion_config::LastFmListsConfig::default(),
            },
            ..AppConfig::default()
        };

        let provider = SpotifyPlaylistListProvider::from_config(&config);
        let artists = provider.fetch_followed_artists().await.unwrap();
        let albums = provider.fetch_saved_albums().await.unwrap();

        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].external_id, "artist-1");
        assert_eq!(artists[0].name, "Artist One");

        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].external_id, "album-1");
        assert_eq!(albums[0].name, "Album One");
        assert_eq!(albums[0].artist_name.as_deref(), Some("Artist One"));
    }

    #[tokio::test]
    async fn spotify_provider_health_check_reflects_missing_token() {
        let config = AppConfig {
            lists: chorrosion_config::ListsConfig {
                musicbrainz: chorrosion_config::MusicBrainzListsConfig::default(),
                spotify: chorrosion_config::SpotifyListsConfig {
                    enabled: true,
                    base_url: None,
                    access_token: None,
                    playlist_ids: vec!["playlist-1".to_string()],
                    market: None,
                },
                lastfm: chorrosion_config::LastFmListsConfig::default(),
            },
            ..AppConfig::default()
        };

        let provider = SpotifyPlaylistListProvider::from_config(&config);
        let health = provider.health_check().await.unwrap();

        assert!(!health.ok);
        assert_eq!(
            health.message.as_deref(),
            Some("Spotify access token not configured")
        );
    }

    #[tokio::test]
    async fn spotify_provider_fetches_paginated_tracks() {
        let server = MockServer::start().await;

        // First page: contains one track and sets `next` to signal more pages.
        Mock::given(method("GET"))
            .and(path("/v1/playlists/playlist-paginated/tracks"))
            .and(query_param("limit", "100"))
            .and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [
                    {
                        "track": {
                            "artists": [
                                {
                                    "id": "artist-page1",
                                    "name": "Artist Page One",
                                    "external_urls": {
                                        "spotify": "https://open.spotify.com/artist/artist-page1"
                                    }
                                }
                            ],
                            "album": {
                                "id": "album-page1",
                                "name": "Album Page One",
                                "artists": [
                                    {
                                        "id": "artist-page1",
                                        "name": "Artist Page One"
                                    }
                                ],
                                "external_urls": {
                                    "spotify": "https://open.spotify.com/album/album-page1"
                                }
                            }
                        }
                    }
                ],
                "next": "https://api.spotify.com/v1/playlists/playlist-paginated/tracks?offset=1&limit=100"
            })))
            .mount(&server)
            .await;

        // Second page: offset=1, no next (last page).
        Mock::given(method("GET"))
            .and(path("/v1/playlists/playlist-paginated/tracks"))
            .and(query_param("limit", "100"))
            .and(query_param("offset", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [
                    {
                        "track": {
                            "artists": [
                                {
                                    "id": "artist-page2",
                                    "name": "Artist Page Two",
                                    "external_urls": {
                                        "spotify": "https://open.spotify.com/artist/artist-page2"
                                    }
                                }
                            ],
                            "album": {
                                "id": "album-page2",
                                "name": "Album Page Two",
                                "artists": [
                                    {
                                        "id": "artist-page2",
                                        "name": "Artist Page Two"
                                    }
                                ],
                                "external_urls": {
                                    "spotify": "https://open.spotify.com/album/album-page2"
                                }
                            }
                        }
                    }
                ],
                "next": null
            })))
            .mount(&server)
            .await;

        let config = AppConfig {
            lists: chorrosion_config::ListsConfig {
                musicbrainz: chorrosion_config::MusicBrainzListsConfig::default(),
                spotify: chorrosion_config::SpotifyListsConfig {
                    enabled: true,
                    base_url: Some(format!("{}/v1", server.uri())),
                    access_token: Some("test-token".to_string()),
                    playlist_ids: vec!["playlist-paginated".to_string()],
                    market: None,
                },
                lastfm: chorrosion_config::LastFmListsConfig::default(),
            },
            ..AppConfig::default()
        };

        let provider = SpotifyPlaylistListProvider::from_config(&config);
        let artists = provider.fetch_followed_artists().await.unwrap();
        let albums = provider.fetch_saved_albums().await.unwrap();

        // Both pages should have been fetched: 2 distinct artists and 2 distinct albums.
        assert_eq!(artists.len(), 2);
        assert!(artists.iter().any(|a| a.external_id == "artist-page1"));
        assert!(artists.iter().any(|a| a.external_id == "artist-page2"));

        assert_eq!(albums.len(), 2);
        assert!(albums.iter().any(|a| a.external_id == "album-page1"));
        assert!(albums.iter().any(|a| a.external_id == "album-page2"));
    }

    #[tokio::test]
    async fn lastfm_provider_imports_artists_and_albums() {
        let server = MockServer::start().await;
        let base_url = format!("{}/2.0/", server.uri());

        Mock::given(method("GET"))
            .and(path("/2.0/"))
            .and(query_param("method", "artist.getinfo"))
            .and(query_param("artist", "Artist One"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "artist": {
                    "name": "Artist One",
                    "bio": { "summary": "Test bio" },
                    "tags": { "tag": [{"name": "rock"}] }
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/2.0/"))
            .and(query_param("method", "album.getinfo"))
            .and(query_param("artist", "Artist One"))
            .and(query_param("album", "Album One"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "album": {
                    "name": "Album One",
                    "artist": "Artist One",
                    "tracks": { "track": [{"name": "Track 1"}, {"name": "Track 2"}] }
                }
            })))
            .mount(&server)
            .await;

        let config = AppConfig {
            lists: chorrosion_config::ListsConfig {
                musicbrainz: chorrosion_config::MusicBrainzListsConfig::default(),
                spotify: chorrosion_config::SpotifyListsConfig::default(),
                lastfm: chorrosion_config::LastFmListsConfig {
                    enabled: true,
                    api_key: Some("test-key".to_string()),
                    base_url: Some(base_url),
                    artist_names: vec!["Artist One".to_string()],
                    album_seeds: vec![chorrosion_config::LastFmListsAlbumSeed {
                        artist: "Artist One".to_string(),
                        album: "Album One".to_string(),
                    }],
                },
            },
            ..AppConfig::default()
        };

        let provider = LastFmListProvider::from_config(&config);
        let artists = provider.fetch_followed_artists().await.unwrap();
        let albums = provider.fetch_saved_albums().await.unwrap();

        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].external_id, "artist one");
        assert_eq!(artists[0].name, "Artist One");

        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].external_id, "artist one::album one");
        assert_eq!(albums[0].name, "Album One");
        assert_eq!(albums[0].artist_name.as_deref(), Some("Artist One"));
    }

    #[tokio::test]
    async fn auto_add_from_list_entries_creates_missing_artists_and_albums() {
        let artist_repo = InMemoryArtistRepo::default();
        let album_repo = InMemoryAlbumRepo::default();

        let summary = auto_add_from_list_entries(
            &artist_repo,
            &album_repo,
            vec![
                ExternalListEntry {
                    entity_type: ListEntityType::Artist,
                    external_id: "artist:one".to_string(),
                    name: "Artist One".to_string(),
                    artist_name: None,
                    source_url: None,
                    followed_at: None,
                },
                ExternalListEntry {
                    entity_type: ListEntityType::Artist,
                    external_id: "artist:one".to_string(),
                    name: "Artist One Duplicate".to_string(),
                    artist_name: None,
                    source_url: None,
                    followed_at: None,
                },
            ],
            vec![ExternalListEntry {
                entity_type: ListEntityType::Album,
                external_id: "album:one".to_string(),
                name: "Album One".to_string(),
                artist_name: Some("Artist One".to_string()),
                source_url: None,
                followed_at: None,
            }],
        )
        .await
        .unwrap();

        assert_eq!(summary.artists_created, 1);
        assert_eq!(summary.albums_created, 1);
        assert_eq!(summary.artists_skipped, 0);
        assert_eq!(summary.albums_skipped_missing_artist, 0);
    }

    #[tokio::test]
    async fn auto_add_from_list_entries_skips_existing_and_missing_artist_name() {
        let artist_repo = InMemoryArtistRepo::default();
        let album_repo = InMemoryAlbumRepo::default();

        let mut existing_artist = Artist::new("Artist Existing");
        existing_artist.foreign_artist_id = Some("artist:existing".to_string());
        let existing_artist = artist_repo.create(existing_artist).await.unwrap();

        let mut existing_album = Album::new(existing_artist.id, "Album Existing");
        existing_album.foreign_album_id = Some("album:existing".to_string());
        album_repo.create(existing_album).await.unwrap();

        let summary = auto_add_from_list_entries(
            &artist_repo,
            &album_repo,
            vec![ExternalListEntry {
                entity_type: ListEntityType::Artist,
                external_id: "artist:existing".to_string(),
                name: "Artist Existing".to_string(),
                artist_name: None,
                source_url: None,
                followed_at: None,
            }],
            vec![
                ExternalListEntry {
                    entity_type: ListEntityType::Album,
                    external_id: "album:existing".to_string(),
                    name: "Album Existing".to_string(),
                    artist_name: Some("Artist Existing".to_string()),
                    source_url: None,
                    followed_at: None,
                },
                ExternalListEntry {
                    entity_type: ListEntityType::Album,
                    external_id: "album:no-artist".to_string(),
                    name: "Album Missing Artist".to_string(),
                    artist_name: None,
                    source_url: None,
                    followed_at: None,
                },
            ],
        )
        .await
        .unwrap();

        assert_eq!(summary.artists_skipped, 1);
        assert_eq!(summary.albums_skipped, 1);
        assert_eq!(summary.albums_skipped_missing_artist, 1);
        assert_eq!(summary.artists_created, 0);
        assert_eq!(summary.albums_created, 0);
    }

    #[tokio::test]
    async fn auto_add_from_list_entries_updates_artist_foreign_id_when_name_matches() {
        let artist_repo = InMemoryArtistRepo::default();
        let album_repo = InMemoryAlbumRepo::default();

        // Seed an artist that has no foreign_artist_id yet.
        let mut existing_artist = Artist::new("Artist Update");
        existing_artist.foreign_artist_id = None;
        artist_repo.create(existing_artist).await.unwrap();

        let summary = auto_add_from_list_entries(
            &artist_repo,
            &album_repo,
            vec![ExternalListEntry {
                entity_type: ListEntityType::Artist,
                external_id: "artist:update".to_string(),
                // Intentionally different case from the seeded "Artist Update" to verify
                // case-insensitive name matching triggers the update path.
                name: "artist update".to_string(),
                artist_name: None,
                source_url: None,
                followed_at: None,
            }],
            vec![],
        )
        .await
        .unwrap();

        assert_eq!(summary.artists_updated, 1);
        assert_eq!(summary.artists_created, 0);
        assert_eq!(summary.artists_skipped, 0);

        let updated = artist_repo
            .get_by_name("Artist Update")
            .await
            .unwrap()
            .expect("artist should exist");
        assert_eq!(updated.foreign_artist_id.as_deref(), Some("artist:update"));
    }

    #[tokio::test]
    async fn auto_add_from_list_entries_updates_album_foreign_id_when_title_matches() {
        let artist_repo = InMemoryArtistRepo::default();
        let album_repo = InMemoryAlbumRepo::default();

        // Seed an artist with a known foreign id so it is found by foreign-id lookup.
        let mut existing_artist = Artist::new("Artist Album Update");
        existing_artist.foreign_artist_id = Some("artist:album-update".to_string());
        let existing_artist = artist_repo.create(existing_artist).await.unwrap();

        // Seed an album under that artist with a matching title but no foreign_album_id.
        let mut existing_album = Album::new(existing_artist.id, "Album Update");
        existing_album.foreign_album_id = None;
        album_repo.create(existing_album).await.unwrap();

        let summary = auto_add_from_list_entries(
            &artist_repo,
            &album_repo,
            vec![],
            vec![ExternalListEntry {
                entity_type: ListEntityType::Album,
                external_id: "album:update".to_string(),
                // Intentionally different case from the seeded "Album Update" to verify
                // case-insensitive title matching triggers the update path.
                name: "ALBUM UPDATE".to_string(),
                artist_name: Some("Artist Album Update".to_string()),
                source_url: None,
                followed_at: None,
            }],
        )
        .await
        .unwrap();

        assert_eq!(summary.albums_updated, 1);
        assert_eq!(summary.albums_created, 0);
        assert_eq!(summary.albums_skipped, 0);

        let updated = album_repo
            .get_by_artist_and_title(existing_artist.id, "Album Update")
            .await
            .unwrap()
            .expect("album should exist");
        assert_eq!(updated.foreign_album_id.as_deref(), Some("album:update"));
    }

    #[tokio::test]
    async fn lastfm_provider_health_check_reflects_missing_api_key() {
        let config = AppConfig {
            lists: chorrosion_config::ListsConfig {
                musicbrainz: chorrosion_config::MusicBrainzListsConfig::default(),
                spotify: chorrosion_config::SpotifyListsConfig::default(),
                lastfm: chorrosion_config::LastFmListsConfig {
                    enabled: true,
                    api_key: None,
                    base_url: None,
                    artist_names: vec!["Artist One".to_string()],
                    album_seeds: vec![],
                },
            },
            ..AppConfig::default()
        };

        let provider = LastFmListProvider::from_config(&config);
        let health = provider.health_check().await.unwrap();

        assert!(!health.ok);
        assert_eq!(
            health.message.as_deref(),
            Some("Last.fm API key not configured")
        );
    }

    #[tokio::test]
    async fn lastfm_provider_health_check_reflects_no_entries_configured() {
        let config = AppConfig {
            lists: chorrosion_config::ListsConfig {
                musicbrainz: chorrosion_config::MusicBrainzListsConfig::default(),
                spotify: chorrosion_config::SpotifyListsConfig::default(),
                lastfm: chorrosion_config::LastFmListsConfig {
                    enabled: true,
                    api_key: Some("test-key".to_string()),
                    base_url: None,
                    artist_names: vec![],
                    album_seeds: vec![],
                },
            },
            ..AppConfig::default()
        };

        let provider = LastFmListProvider::from_config(&config);
        let health = provider.health_check().await.unwrap();

        assert!(!health.ok);
        assert_eq!(
            health.message.as_deref(),
            Some("no Last.fm artist names or album seeds configured")
        );
    }

    #[tokio::test]
    async fn lastfm_provider_source_urls_are_percent_encoded() {
        let server = MockServer::start().await;
        let base_url = format!("{}/2.0/", server.uri());

        Mock::given(method("GET"))
            .and(path("/2.0/"))
            .and(query_param("method", "artist.getinfo"))
            .and(query_param("artist", "AC/DC"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "artist": {
                    "name": "AC/DC",
                    "bio": { "summary": "" },
                    "tags": { "tag": [] }
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/2.0/"))
            .and(query_param("method", "album.getinfo"))
            .and(query_param("artist", "AC/DC"))
            .and(query_param("album", "Back in Black"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "album": {
                    "name": "Back in Black",
                    "artist": "AC/DC",
                    "tracks": { "track": [] }
                }
            })))
            .mount(&server)
            .await;

        let config = AppConfig {
            lists: chorrosion_config::ListsConfig {
                musicbrainz: chorrosion_config::MusicBrainzListsConfig::default(),
                spotify: chorrosion_config::SpotifyListsConfig::default(),
                lastfm: chorrosion_config::LastFmListsConfig {
                    enabled: true,
                    api_key: Some("test-key".to_string()),
                    base_url: Some(base_url),
                    artist_names: vec!["AC/DC".to_string()],
                    album_seeds: vec![chorrosion_config::LastFmListsAlbumSeed {
                        artist: "AC/DC".to_string(),
                        album: "Back in Black".to_string(),
                    }],
                },
            },
            ..AppConfig::default()
        };

        let provider = LastFmListProvider::from_config(&config);
        let artists = provider.fetch_followed_artists().await.unwrap();
        let albums = provider.fetch_saved_albums().await.unwrap();

        assert_eq!(artists.len(), 1);
        assert_eq!(
            artists[0].source_url.as_deref(),
            Some("https://www.last.fm/music/AC%2FDC")
        );

        assert_eq!(albums.len(), 1);
        assert_eq!(
            albums[0].source_url.as_deref(),
            Some("https://www.last.fm/music/AC%2FDC/Back%20in%20Black")
        );
    }
}
