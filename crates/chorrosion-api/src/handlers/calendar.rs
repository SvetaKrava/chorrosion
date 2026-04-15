// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chorrosion_application::AppState;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use tracing::debug;
use utoipa::{IntoParams, ToSchema};

// ============================================================================
// Query params
// ============================================================================

#[derive(Debug, Deserialize, IntoParams)]
pub struct CalendarQuery {
    /// Start date (inclusive) in ``YYYY-MM-DD`` format. Defaults to today.
    pub start: Option<String>,
    /// End date (inclusive) in ``YYYY-MM-DD`` format. Defaults to 30 days from today.
    pub end: Option<String>,
}

// ============================================================================
// Response types
// ============================================================================

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CalendarAlbumResponse {
    pub id: String,
    pub artist_id: String,
    pub artist_name: String,
    pub title: String,
    pub release_date: String,
    pub album_type: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CalendarResponse {
    pub items: Vec<CalendarAlbumResponse>,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CalendarErrorResponse {
    pub error: String,
}

// ============================================================================
// Helpers
// ============================================================================

fn parse_date_param(
    s: Option<&str>,
    fallback: NaiveDate,
    label: &str,
) -> Result<NaiveDate, (StatusCode, Json<CalendarErrorResponse>)> {
    match s {
        None => Ok(fallback),
        Some(v) => NaiveDate::parse_from_str(v, "%Y-%m-%d").map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(CalendarErrorResponse {
                    error: format!(
                        "invalid date for '{}': '{}' (expected YYYY-MM-DD)",
                        label, v
                    ),
                }),
            )
        }),
    }
}

/// Escape a string for use in an iCal text value.
/// Escapes backslashes, semicolons, commas, and newlines per RFC 5545 §3.3.11.
fn ical_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
        .replace('\r', "")
}

/// Fold long lines per RFC 5545 §3.1 (max 75 octets, fold with CRLF + SPACE).
fn ical_fold(line: &str) -> String {
    let bytes = line.as_bytes();
    if bytes.len() <= 75 {
        return format!("{line}\r\n");
    }
    let mut result = String::new();
    let mut pos = 0;
    let mut first = true;
    while pos < bytes.len() {
        let max = if first { 75 } else { 74 };
        // Advance by up to `max` bytes, staying on a char boundary.
        let mut end = (pos + max).min(bytes.len());
        while end > pos && !line.is_char_boundary(end) {
            end -= 1;
        }
        if first {
            result.push_str(&line[pos..end]);
            result.push_str("\r\n");
            first = false;
        } else {
            result.push(' ');
            result.push_str(&line[pos..end]);
            result.push_str("\r\n");
        }
        pos = end;
    }
    result
}

// ============================================================================
// Handlers
// ============================================================================

#[utoipa::path(
    get,
    path = "/api/v1/calendar",
    params(CalendarQuery),
    responses(
        (status = 200, description = "Upcoming releases in the requested date window", body = CalendarResponse),
        (status = 400, description = "Invalid date parameters", body = CalendarErrorResponse),
        (status = 500, description = "Internal server error", body = CalendarErrorResponse),
    ),
    tag = "calendar"
)]
pub async fn list_upcoming_releases(
    State(state): State<AppState>,
    Query(query): Query<CalendarQuery>,
) -> Result<Json<CalendarResponse>, (StatusCode, Json<CalendarErrorResponse>)> {
    let today = Utc::now().date_naive();
    let default_end = today + chrono::Duration::days(30);

    let start = parse_date_param(query.start.as_deref(), today, "start")?;
    let end = parse_date_param(query.end.as_deref(), default_end, "end")?;

    if end < start {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(CalendarErrorResponse {
                error: "end date must not be before start date".to_string(),
            }),
        ));
    }

    debug!(target: "api", %start, %end, "listing upcoming releases");

    let albums = state
        .album_repository
        .list_upcoming_releases(start, end, 5000, 0)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CalendarErrorResponse {
                    error: format!("failed to list upcoming releases: {e}"),
                }),
            )
        })?;

    let mut items = Vec::with_capacity(albums.len());
    let mut artist_cache: HashMap<String, String> = HashMap::new();
    for album in albums {
        let artist_id_str = album.artist_id.to_string();
        let artist_name = match artist_cache.entry(artist_id_str) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let name = state
                    .artist_repository
                    .get_by_id(e.key().clone())
                    .await
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(CalendarErrorResponse {
                                error: format!("failed to fetch artist: {e}"),
                            }),
                        )
                    })?
                    .map(|a| a.name)
                    .unwrap_or_else(|| "Unknown Artist".to_string());
                e.insert(name).clone()
            }
        };

        items.push(CalendarAlbumResponse {
            id: album.id.to_string(),
            artist_id: album.artist_id.to_string(),
            artist_name,
            title: album.title,
            release_date: album
                .release_date
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
            album_type: album.album_type,
            status: album.status.to_string(),
        });
    }

    Ok(Json(CalendarResponse {
        items,
        start: start.format("%Y-%m-%d").to_string(),
        end: end.format("%Y-%m-%d").to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/calendar/ical",
    params(CalendarQuery),
    responses(
        (status = 200, description = "iCal feed of upcoming releases (text/calendar)", content_type = "text/calendar"),
        (status = 400, description = "Invalid date parameters", body = CalendarErrorResponse),
        (status = 500, description = "Internal server error", body = CalendarErrorResponse),
    ),
    tag = "calendar"
)]
pub async fn get_ical_feed(
    State(state): State<AppState>,
    Query(query): Query<CalendarQuery>,
) -> Response {
    let today = Utc::now().date_naive();
    let default_end = today + chrono::Duration::days(30);

    let start = match parse_date_param(query.start.as_deref(), today, "start") {
        Ok(d) => d,
        Err((status, body)) => return (status, body).into_response(),
    };
    let end = match parse_date_param(query.end.as_deref(), default_end, "end") {
        Ok(d) => d,
        Err((status, body)) => return (status, body).into_response(),
    };

    if end < start {
        return (
            StatusCode::BAD_REQUEST,
            Json(CalendarErrorResponse {
                error: "end date must not be before start date".to_string(),
            }),
        )
            .into_response();
    }

    debug!(target: "api", %start, %end, "generating ical feed");

    let albums = match state
        .album_repository
        .list_upcoming_releases(start, end, 5000, 0)
        .await
    {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CalendarErrorResponse {
                    error: format!("failed to list upcoming releases: {e}"),
                }),
            )
                .into_response();
        }
    };

    let mut cal = String::from(
        "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Chorrosion//Music Releases//EN\r\n\
CALSCALE:GREGORIAN\r\n\
METHOD:PUBLISH\r\n\
X-WR-CALNAME:Chorrosion Music Releases\r\n",
    );

    let mut artist_cache: HashMap<String, String> = HashMap::new();
    for album in &albums {
        let artist_id_str = album.artist_id.to_string();
        let artist_name = match artist_cache.entry(artist_id_str) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let name = match state.artist_repository.get_by_id(e.key().clone()).await {
                    Ok(artist) => artist
                        .map(|a| a.name)
                        .unwrap_or_else(|| "Unknown Artist".to_string()),
                    Err(e) => {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(CalendarErrorResponse {
                                error: format!("failed to fetch artist: {e}"),
                            }),
                        )
                            .into_response();
                    }
                };
                e.insert(name).clone()
            }
        };

        let release_str = album
            .release_date
            .map(|d| d.format("%Y%m%d").to_string())
            .unwrap_or_default();

        let summary = ical_escape(&format!("{} - {}", artist_name, album.title));
        let description = {
            let mut parts = vec![
                format!("Artist: {}", ical_escape(&artist_name)),
                format!("Album: {}", ical_escape(&album.title)),
            ];
            if let Some(at) = &album.album_type {
                parts.push(format!("Type: {}", ical_escape(at)));
            }
            parts.join("\\n")
        };
        let uid = format!("{}@chorrosion", album.id);

        cal.push_str("BEGIN:VEVENT\r\n");
        cal.push_str(&ical_fold(&format!("UID:{}", uid)));
        cal.push_str(&ical_fold(&format!("DTSTART;VALUE=DATE:{}", release_str)));
        cal.push_str(&ical_fold(&format!("SUMMARY:{}", summary)));
        cal.push_str(&ical_fold(&format!("DESCRIPTION:{}", description)));
        cal.push_str("END:VEVENT\r\n");
    }

    cal.push_str("END:VCALENDAR\r\n");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"chorrosion.ics\"",
        )
        .body(axum::body::Body::from(cal))
        .unwrap()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use chorrosion_config::AppConfig;
    use chorrosion_domain::{Album, AlbumStatus, Artist, ArtistId};
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackRepository,
    };
    use sqlx::SqlitePool;
    use std::sync::Arc;

    async fn make_test_pool_and_state() -> (SqlitePool, AppState) {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to open in-memory sqlite");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations failed");
        let state = AppState::new(
            AppConfig::default(),
            Arc::new(SqliteArtistRepository::new(pool.clone())),
            Arc::new(SqliteAlbumRepository::new(pool.clone())),
            Arc::new(SqliteTrackRepository::new(pool.clone())),
            Arc::new(SqliteQualityProfileRepository::new(pool.clone())),
            Arc::new(SqliteMetadataProfileRepository::new(pool.clone())),
            Arc::new(SqliteIndexerDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteDownloadClientDefinitionRepository::new(pool.clone())),
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        );
        (pool, state)
    }

    fn make_artist(name: &str) -> Artist {
        Artist::new(name)
    }

    fn make_album_with_date(artist_id: ArtistId, title: &str, date: NaiveDate) -> Album {
        let mut a = Album::new(artist_id, title);
        a.status = AlbumStatus::Released;
        a.monitored = true;
        a.release_date = Some(date);
        a
    }

    #[tokio::test]
    async fn list_upcoming_releases_returns_albums_in_window() {
        let (_pool, state) = make_test_pool_and_state().await;

        let artist = state
            .artist_repository
            .create(make_artist("Test Artist"))
            .await
            .unwrap();

        let in_window_date = NaiveDate::from_ymd_opt(2030, 6, 15).unwrap();
        let out_of_window_date = NaiveDate::from_ymd_opt(2030, 8, 1).unwrap();

        state
            .album_repository
            .create(make_album_with_date(
                artist.id,
                "In Window Album",
                in_window_date,
            ))
            .await
            .unwrap();
        state
            .album_repository
            .create(make_album_with_date(
                artist.id,
                "Out of Window Album",
                out_of_window_date,
            ))
            .await
            .unwrap();

        let query = CalendarQuery {
            start: Some("2030-06-01".to_string()),
            end: Some("2030-06-30".to_string()),
        };
        let result = list_upcoming_releases(State(state), Query(query))
            .await
            .expect("expected 200");

        let body = result.0;
        assert_eq!(body.items.len(), 1);
        assert_eq!(body.items[0].title, "In Window Album");
        assert_eq!(body.items[0].artist_name, "Test Artist");
        assert_eq!(body.start, "2030-06-01");
        assert_eq!(body.end, "2030-06-30");
    }

    #[tokio::test]
    async fn list_upcoming_releases_empty_window_returns_empty() {
        let (_pool, state) = make_test_pool_and_state().await;

        let query = CalendarQuery {
            start: Some("2030-01-01".to_string()),
            end: Some("2030-01-31".to_string()),
        };
        let result = list_upcoming_releases(State(state), Query(query))
            .await
            .expect("expected 200");

        assert_eq!(result.0.items.len(), 0);
    }

    #[tokio::test]
    async fn list_upcoming_releases_bad_date_returns_400() {
        let (_pool, state) = make_test_pool_and_state().await;

        let query = CalendarQuery {
            start: Some("not-a-date".to_string()),
            end: None,
        };
        let result = list_upcoming_releases(State(state), Query(query)).await;
        let (status, _) = result.expect_err("expected error");
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_upcoming_releases_end_before_start_returns_400() {
        let (_pool, state) = make_test_pool_and_state().await;

        let query = CalendarQuery {
            start: Some("2030-06-30".to_string()),
            end: Some("2030-06-01".to_string()),
        };
        let result = list_upcoming_releases(State(state), Query(query)).await;
        let (status, _) = result.expect_err("expected error");
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn ical_feed_returns_text_calendar_content_type() {
        let (_pool, state) = make_test_pool_and_state().await;

        let query = CalendarQuery {
            start: Some("2030-01-01".to_string()),
            end: Some("2030-01-31".to_string()),
        };
        let response = get_ical_feed(State(state), Query(query)).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/calendar; charset=utf-8"
        );
        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body_bytes).unwrap();
        assert!(body_str.contains("BEGIN:VCALENDAR"));
        assert!(body_str.contains("END:VCALENDAR"));
    }

    #[tokio::test]
    async fn ical_feed_includes_vevent_for_album() {
        let (_pool, state) = make_test_pool_and_state().await;

        let artist = state
            .artist_repository
            .create(make_artist("Calendar Artist"))
            .await
            .unwrap();

        let release_date = NaiveDate::from_ymd_opt(2030, 3, 15).unwrap();
        state
            .album_repository
            .create(make_album_with_date(
                artist.id,
                "Calendar Album",
                release_date,
            ))
            .await
            .unwrap();

        let query = CalendarQuery {
            start: Some("2030-03-01".to_string()),
            end: Some("2030-03-31".to_string()),
        };
        let response = get_ical_feed(State(state), Query(query)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body_bytes).unwrap();
        assert!(body_str.contains("BEGIN:VEVENT"));
        assert!(body_str.contains("DTSTART;VALUE=DATE:20300315"));
        assert!(body_str.contains("Calendar Artist"));
        assert!(body_str.contains("Calendar Album"));
    }
}
