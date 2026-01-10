// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(test)]
mod tests {
    use crate::{MusicBrainzClient, SearchQuery};
    use uuid::Uuid;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const RADIOHEAD_MBID: &str = "a74b1b7f-71a5-4011-9441-d0b5e4122711";
    const OK_COMPUTER_MBID: &str = "b1392450-e666-3926-a536-22c65f834433";
    const PARANOID_ANDROID_RECORDING_MBID: &str = "e5a3f0c4-1fae-4f2e-8f76-0c3b4f1e4fa6";

    fn artist_search_response() -> serde_json::Value {
        serde_json::json!({
            "created": "2026-01-08T12:00:00.000Z",
            "count": 1,
            "offset": 0,
            "artists": [{
                "id": RADIOHEAD_MBID,
                "name": "Radiohead",
                "sort-name": "Radiohead",
                "type": "Group",
                "country": "GB",
                "disambiguation": "",
                "score": 100
            }]
        })
    }

    fn artist_lookup_response() -> serde_json::Value {
        serde_json::json!({
            "id": RADIOHEAD_MBID,
            "name": "Radiohead",
            "sort-name": "Radiohead",
            "type": "Group",
            "country": "GB"
        })
    }

    fn album_search_response() -> serde_json::Value {
        serde_json::json!({
            "created": "2026-01-08T12:00:00.000Z",
            "count": 1,
            "offset": 0,
            "release-groups": [{
                "id": OK_COMPUTER_MBID,
                "title": "OK Computer",
                "primary-type": "Album",
                "secondary-types": [],
                "first-release-date": "1997-05-21",
                "artist-credit": [{
                    "name": "Radiohead",
                    "artist": {
                        "id": RADIOHEAD_MBID,
                        "name": "Radiohead",
                        "sort-name": "Radiohead"
                    }
                }],
                "score": 100
            }]
        })
    }

    fn album_lookup_response() -> serde_json::Value {
        serde_json::json!({
            "id": OK_COMPUTER_MBID,
            "title": "OK Computer",
            "primary-type": "Album",
            "secondary-types": [],
            "first-release-date": "1997-05-21",
            "artist-credit": [{
                "name": "Radiohead",
                "artist": {
                    "id": RADIOHEAD_MBID,
                    "name": "Radiohead",
                    "sort-name": "Radiohead"
                }
            }]
        })
    }

    fn recording_lookup_response() -> serde_json::Value {
        serde_json::json!({
            "id": PARANOID_ANDROID_RECORDING_MBID,
            "title": "Paranoid Android",
            "length": 387000,
            "artist-credit": [{
                "name": "Radiohead",
                "artist": {
                    "id": RADIOHEAD_MBID,
                    "name": "Radiohead",
                    "sort-name": "Radiohead"
                }
            }],
            "releases": [{
                "id": OK_COMPUTER_MBID,
                "title": "OK Computer",
                "status": "Official",
                "country": "GB",
                "date": "1997-05-21",
                "release-group": {
                    "id": OK_COMPUTER_MBID,
                    "title": "OK Computer",
                    "primary-type": "Album"
                }
            }]
        })
    }

    fn cover_art_response() -> serde_json::Value {
        serde_json::json!({
            "images": [{
                "image": "https://coverartarchive.org/release-group/b1392450-e666-3926-a536-22c65f834433/front.jpg",
                "front": true,
                "back": false,
                "approved": true,
                "types": ["Front"],
                "thumbnails": {
                    "250": "https://coverartarchive.org/release-group/b1392450/front-250.jpg",
                    "500": "https://coverartarchive.org/release-group/b1392450/front-500.jpg"
                }
            }]
        })
    }

    #[tokio::test]
    async fn test_search_artists() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/artist"))
            .and(query_param("query", "Radiohead"))
            .and(query_param("fmt", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(artist_search_response()))
            .mount(&mock_server)
            .await;

        let client = MusicBrainzClient::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let query = SearchQuery::new("Radiohead");
        let response = client.search_artists(query).await.unwrap();

        assert_eq!(response.count, 1);
        assert_eq!(response.results.artists.len(), 1);

        let artist = &response.results.artists[0];
        assert_eq!(artist.name, "Radiohead");
        assert_eq!(artist.id, Uuid::parse_str(RADIOHEAD_MBID).unwrap());
        assert_eq!(artist.country, Some("GB".to_string()));
    }

    #[tokio::test]
    async fn test_search_artists_with_pagination() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/artist"))
            .and(query_param("query", "John"))
            .and(query_param("limit", "5"))
            .and(query_param("offset", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(artist_search_response()))
            .mount(&mock_server)
            .await;

        let client = MusicBrainzClient::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let query = SearchQuery::new("John").limit(5).offset(10);
        let _response = client.search_artists(query).await.unwrap();
    }

    #[tokio::test]
    async fn test_lookup_artist() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(format!("/artist/{}", RADIOHEAD_MBID)))
            .and(query_param("fmt", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(artist_lookup_response()))
            .mount(&mock_server)
            .await;

        let client = MusicBrainzClient::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let mbid = Uuid::parse_str(RADIOHEAD_MBID).unwrap();
        let artist = client.lookup_artist(mbid).await.unwrap();

        assert_eq!(artist.name, "Radiohead");
        assert_eq!(artist.id, mbid);
        assert_eq!(artist.country, Some("GB".to_string()));
    }

    #[tokio::test]
    async fn test_search_albums() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/release-group"))
            .and(query_param("query", "OK Computer"))
            .and(query_param("fmt", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(album_search_response()))
            .mount(&mock_server)
            .await;

        let client = MusicBrainzClient::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let query = SearchQuery::new("OK Computer");
        let response = client.search_albums(query).await.unwrap();

        assert_eq!(response.count, 1);
        assert_eq!(response.results.release_groups.len(), 1);

        let album = &response.results.release_groups[0];
        assert_eq!(album.title, "OK Computer");
        assert_eq!(album.id, Uuid::parse_str(OK_COMPUTER_MBID).unwrap());
        assert_eq!(album.first_release_date, Some("1997-05-21".to_string()));
    }

    #[tokio::test]
    async fn test_lookup_album() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(format!("/release-group/{}", OK_COMPUTER_MBID)))
            .and(query_param("fmt", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(album_lookup_response()))
            .mount(&mock_server)
            .await;

        let client = MusicBrainzClient::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let mbid = Uuid::parse_str(OK_COMPUTER_MBID).unwrap();
        let album = client.lookup_album(mbid).await.unwrap();

        assert_eq!(album.title, "OK Computer");
        assert_eq!(album.id, mbid);
        assert_eq!(album.primary_type, Some("Album".to_string()));
    }

    #[tokio::test]
    async fn test_lookup_recording() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/recording/{}",
                PARANOID_ANDROID_RECORDING_MBID
            )))
            .and(query_param("fmt", "json"))
            .and(query_param("inc", "artists releases release-groups"))
            .respond_with(ResponseTemplate::new(200).set_body_json(recording_lookup_response()))
            .mount(&mock_server)
            .await;

        let client = MusicBrainzClient::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let mbid = Uuid::parse_str(PARANOID_ANDROID_RECORDING_MBID).unwrap();
        let recording = client.lookup_recording(mbid).await.unwrap();

        assert_eq!(recording.id, mbid);
        assert_eq!(recording.title, "Paranoid Android");
        assert_eq!(recording.artist_credit.len(), 1);
        assert_eq!(recording.releases.len(), 1);
        assert_eq!(
            recording.releases[0].release_group.id,
            Uuid::parse_str(OK_COMPUTER_MBID).unwrap()
        );
    }

    #[tokio::test]
    async fn test_fetch_cover_art_cached() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(format!("/release-group/{}", OK_COMPUTER_MBID)))
            .respond_with(ResponseTemplate::new(200).set_body_json(cover_art_response()))
            .mount(&mock_server)
            .await;

        let client = MusicBrainzClient::builder()
            .base_url(mock_server.uri())
            .cover_art_base_url(mock_server.uri())
            .build()
            .unwrap();

        let mbid = Uuid::parse_str(OK_COMPUTER_MBID).unwrap();

        let art_first = client.fetch_cover_art(mbid).await.unwrap();
        assert_eq!(art_first.images.len(), 1);
        assert!(art_first.images[0].front);

        let art_second = client.fetch_cover_art(mbid).await.unwrap();
        assert_eq!(art_second.images.len(), 1);

        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1, "expected cover art fetch to be cached");
    }

    #[tokio::test]
    async fn test_not_found_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(format!("/artist/{}", RADIOHEAD_MBID)))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = MusicBrainzClient::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let mbid = Uuid::parse_str(RADIOHEAD_MBID).unwrap();
        let result = client.lookup_artist(mbid).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::MusicBrainzError::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn test_rate_limit_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/artist"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let client = MusicBrainzClient::builder()
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let query = SearchQuery::new("Test");
        let result = client.search_artists(query).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::MusicBrainzError::RateLimitExceeded
        ));
    }
}
