// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{http::StatusCode, response::IntoResponse, Json};
use chorrosion_application::{
    evaluate_import_match, parse_track_metadata, CatalogAlbum, CatalogAlbumMatch, ImportDecision,
    ImportMatchingError, MatchStrategy, MetadataSource, RawTrackMetadata,
};
use chorrosion_domain::{AlbumId, ArtistId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ImportErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ImportCandidateRequest {
    pub raw_metadata: ImportRawMetadataRequest,
    pub catalog: Vec<ImportCatalogAlbumRequest>,
    #[serde(default = "default_fuzzy_threshold")]
    pub fuzzy_threshold: f32,
    #[serde(default = "default_auto_import_threshold")]
    pub auto_import_threshold: f32,
}

fn default_fuzzy_threshold() -> f32 {
    0.7
}

fn default_auto_import_threshold() -> f32 {
    0.8
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ImportRawMetadataRequest {
    pub file_path: String,
    pub embedded_artist: Option<String>,
    pub embedded_album: Option<String>,
    pub embedded_title: Option<String>,
    pub duration_seconds: Option<u32>,
    pub bitrate_kbps: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ImportCatalogAlbumRequest {
    pub artist_id: String,
    pub album_id: String,
    pub artist_name: String,
    pub album_title: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ImportCandidateResponse {
    pub parsed_metadata: ParsedMetadataResponse,
    pub best_match: Option<CatalogAlbumMatchResponse>,
    pub decision: ImportDecisionResponse,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ParsedMetadataResponse {
    pub file_path: String,
    pub artist: String,
    pub album: String,
    pub title: String,
    pub duration_seconds: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub source: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CatalogAlbumMatchResponse {
    pub artist_id: String,
    pub album_id: String,
    pub confidence: f32,
    pub strategy: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ImportDecisionResponse {
    pub decision_type: String,
    pub artist_id: Option<String>,
    pub album_id: Option<String>,
    pub confidence: Option<f32>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ManualImportDecisionRequest {
    pub action: String,
    pub artist_id: Option<String>,
    pub album_id: Option<String>,
    pub confidence: Option<f32>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ManualImportDecisionResponse {
    pub accepted: bool,
    pub decision: ImportDecisionResponse,
}

#[utoipa::path(
    post,
    path = "/api/v1/imports/evaluate",
    request_body = ImportCandidateRequest,
    responses(
        (status = 200, description = "Import candidate evaluated", body = ImportCandidateResponse),
        (status = 400, description = "Invalid request", body = ImportErrorResponse)
    ),
    tag = "imports"
)]
pub async fn evaluate_import_candidate(
    Json(request): Json<ImportCandidateRequest>,
) -> Result<Json<ImportCandidateResponse>, (StatusCode, Json<ImportErrorResponse>)> {
    if !(0.0..=1.0).contains(&request.fuzzy_threshold) {
        return Err(bad_request("fuzzy_threshold must be between 0.0 and 1.0"));
    }

    if !(0.0..=1.0).contains(&request.auto_import_threshold) {
        return Err(bad_request(
            "auto_import_threshold must be between 0.0 and 1.0",
        ));
    }

    // Validate catalog UUIDs before doing any filesystem I/O.
    let catalog = request
        .catalog
        .into_iter()
        .map(|item| {
            Ok(CatalogAlbum {
                artist_id: parse_artist_id(&item.artist_id)?,
                album_id: parse_album_id(&item.album_id)?,
                artist_name: item.artist_name,
                album_title: item.album_title,
            })
        })
        .collect::<Result<Vec<_>, (StatusCode, Json<ImportErrorResponse>)>>()?;

    let raw = RawTrackMetadata {
        file_path: request.raw_metadata.file_path.into(),
        embedded_artist: request.raw_metadata.embedded_artist,
        embedded_album: request.raw_metadata.embedded_album,
        embedded_title: request.raw_metadata.embedded_title,
        duration_seconds: request.raw_metadata.duration_seconds,
        bitrate_kbps: request.raw_metadata.bitrate_kbps,
    };

    let parsed = parse_track_metadata(&raw).await.map_err(|e| match e {
        ImportMatchingError::PathNotFound(_) => bad_request("file not found"),
        ImportMatchingError::Io(_) => bad_request("unable to read file"),
        ImportMatchingError::MetadataParsing(msg) => bad_request(&msg),
    })?;

    let evaluation = evaluate_import_match(
        &parsed,
        &catalog,
        request.fuzzy_threshold,
        request.auto_import_threshold,
    );

    Ok(Json(ImportCandidateResponse {
        parsed_metadata: ParsedMetadataResponse {
            file_path: parsed.file_path.display().to_string(),
            artist: parsed.artist,
            album: parsed.album,
            title: parsed.title,
            duration_seconds: parsed.duration_seconds,
            bitrate_kbps: parsed.bitrate_kbps,
            source: map_metadata_source(&parsed.source).to_string(),
        },
        best_match: evaluation.best_match.map(map_best_match),
        decision: map_decision(evaluation.decision),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/imports/decision",
    request_body = ManualImportDecisionRequest,
    responses(
        (status = 200, description = "Manual import decision accepted", body = ManualImportDecisionResponse),
        (status = 400, description = "Invalid request", body = ImportErrorResponse)
    ),
    tag = "imports"
)]
pub async fn submit_manual_import_decision(
    Json(request): Json<ManualImportDecisionRequest>,
) -> impl IntoResponse {
    let action = request.action.trim().to_ascii_lowercase();

    let decision = match action.as_str() {
        "import" => {
            let artist_id = match request.artist_id {
                Some(value) if !value.trim().is_empty() => match parse_artist_id(value.trim()) {
                    Ok(id) => id,
                    Err(e) => return e.into_response(),
                },
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ImportErrorResponse {
                            error: "artist_id is required for import action".to_string(),
                        }),
                    )
                        .into_response();
                }
            };
            let album_id = match request.album_id {
                Some(value) if !value.trim().is_empty() => match parse_album_id(value.trim()) {
                    Ok(id) => id,
                    Err(e) => return e.into_response(),
                },
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ImportErrorResponse {
                            error: "album_id is required for import action".to_string(),
                        }),
                    )
                        .into_response();
                }
            };

            if let Some(c) = request.confidence {
                if !(0.0..=1.0).contains(&c) {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ImportErrorResponse {
                            error: "confidence must be between 0.0 and 1.0".to_string(),
                        }),
                    )
                        .into_response();
                }
            }

            ImportDecisionResponse {
                decision_type: "import".to_string(),
                artist_id: Some(artist_id.to_string()),
                album_id: Some(album_id.to_string()),
                confidence: request.confidence,
                reason: None,
            }
        }
        "skip" => ImportDecisionResponse {
            decision_type: "skip".to_string(),
            artist_id: None,
            album_id: None,
            confidence: None,
            reason: request.reason.or_else(|| Some("manual skip".to_string())),
        },
        "needs_review" => ImportDecisionResponse {
            decision_type: "needs_review".to_string(),
            artist_id: None,
            album_id: None,
            confidence: request.confidence,
            reason: request
                .reason
                .or_else(|| Some("manual review requested".to_string())),
        },
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ImportErrorResponse {
                    error: "action must be one of: import, skip, needs_review".to_string(),
                }),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(ManualImportDecisionResponse {
            accepted: true,
            decision,
        }),
    )
        .into_response()
}

fn map_match_strategy(strategy: &MatchStrategy) -> &'static str {
    match strategy {
        MatchStrategy::Exact => "exact",
        MatchStrategy::Fuzzy => "fuzzy",
    }
}

fn map_metadata_source(source: &MetadataSource) -> &'static str {
    match source {
        MetadataSource::EmbeddedTags => "embedded_tags",
        MetadataSource::FilenameHeuristics => "filename_heuristics",
    }
}

fn map_best_match(best_match: CatalogAlbumMatch) -> CatalogAlbumMatchResponse {
    CatalogAlbumMatchResponse {
        artist_id: best_match.artist_id.to_string(),
        album_id: best_match.album_id.to_string(),
        confidence: best_match.confidence,
        strategy: map_match_strategy(&best_match.strategy).to_string(),
    }
}

fn map_decision(decision: ImportDecision) -> ImportDecisionResponse {
    match decision {
        ImportDecision::Import {
            artist_id,
            album_id,
            confidence,
        } => ImportDecisionResponse {
            decision_type: "import".to_string(),
            artist_id: Some(artist_id.to_string()),
            album_id: Some(album_id.to_string()),
            confidence: Some(confidence),
            reason: None,
        },
        ImportDecision::NeedsReview { reason, confidence } => ImportDecisionResponse {
            decision_type: "needs_review".to_string(),
            artist_id: None,
            album_id: None,
            confidence: Some(confidence),
            reason: Some(reason),
        },
        ImportDecision::Skip { reason } => ImportDecisionResponse {
            decision_type: "skip".to_string(),
            artist_id: None,
            album_id: None,
            confidence: None,
            reason: Some(reason),
        },
    }
}

fn parse_artist_id(id: &str) -> Result<ArtistId, (StatusCode, Json<ImportErrorResponse>)> {
    let parsed = Uuid::parse_str(id).map_err(|_| bad_request("invalid artist_id UUID"))?;
    Ok(ArtistId::from_uuid(parsed))
}

fn parse_album_id(id: &str) -> Result<AlbumId, (StatusCode, Json<ImportErrorResponse>)> {
    let parsed = Uuid::parse_str(id).map_err(|_| bad_request("invalid album_id UUID"))?;
    Ok(AlbumId::from_uuid(parsed))
}

fn bad_request(message: &str) -> (StatusCode, Json<ImportErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ImportErrorResponse {
            error: message.to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        routing::post,
        Router,
    };
    use serde_json::json;
    use tower::util::ServiceExt;

    fn imports_router() -> Router {
        Router::new()
            .route("/api/v1/imports/evaluate", post(evaluate_import_candidate))
            .route(
                "/api/v1/imports/decision",
                post(submit_manual_import_decision),
            )
    }

    async fn post_json(
        app: Router,
        uri: &str,
        body: serde_json::Value,
    ) -> axum::response::Response {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    // ---- evaluate_import_candidate ----

    #[tokio::test]
    async fn evaluate_rejects_fuzzy_threshold_out_of_range() {
        let app = imports_router();
        let body = json!({
            "raw_metadata": {
                "file_path": "/tmp/nonexistent.mp3",
                "embedded_artist": "Artist",
                "embedded_album": "Album",
                "embedded_title": "Title",
                "bitrate_kbps": 320
            },
            "catalog": [],
            "fuzzy_threshold": 1.5,
            "auto_import_threshold": 0.8
        });
        let response = post_json(app, "/api/v1/imports/evaluate", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert!(payload["error"]
            .as_str()
            .unwrap()
            .contains("fuzzy_threshold"));
    }

    #[tokio::test]
    async fn evaluate_rejects_invalid_catalog_uuid() {
        let app = imports_router();
        let body = json!({
            "raw_metadata": {
                "file_path": "/tmp/nonexistent.mp3",
                "embedded_artist": "Artist",
                "embedded_album": "Album",
                "embedded_title": "Title",
                "bitrate_kbps": 320
            },
            "catalog": [{
                "artist_id": "not-a-uuid",
                "album_id": "00000000-0000-0000-0000-000000000001",
                "artist_name": "Artist",
                "album_title": "Album"
            }],
            "fuzzy_threshold": 0.7,
            "auto_import_threshold": 0.8
        });
        let response = post_json(app, "/api/v1/imports/evaluate", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert!(payload["error"].as_str().unwrap().contains("artist_id"));
    }

    #[tokio::test]
    async fn evaluate_returns_file_not_found_without_leaking_path() {
        let app = imports_router();
        let body = json!({
            "raw_metadata": {
                "file_path": "/tmp/chorrosion_test_nonexistent_123456.mp3",
                "embedded_artist": "Artist",
                "embedded_album": "Album",
                "embedded_title": "Title",
                "bitrate_kbps": 320
            },
            "catalog": [],
            "fuzzy_threshold": 0.7,
            "auto_import_threshold": 0.8
        });
        let response = post_json(app, "/api/v1/imports/evaluate", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        let error_msg = payload["error"].as_str().unwrap();
        assert_eq!(error_msg, "file not found");
        assert!(!error_msg.contains("chorrosion_test_nonexistent_123456"));
    }

    #[tokio::test]
    async fn evaluate_happy_path_returns_parsed_metadata_and_decision() {
        use std::io::Write;

        let tmp = tempfile::NamedTempFile::new().expect("temp file");
        tmp.as_file()
            .write_all(b"dummy")
            .expect("write temp file content");
        let path = tmp.path().to_string_lossy().to_string();

        let artist_uuid = "00000000-0000-0000-0000-000000000001";
        let album_uuid = "00000000-0000-0000-0000-000000000002";
        let app = imports_router();
        let body = json!({
            "raw_metadata": {
                "file_path": path,
                "embedded_artist": "Pink Floyd",
                "embedded_album": "The Wall",
                "embedded_title": "Comfortably Numb",
                "bitrate_kbps": 320
            },
            "catalog": [{
                "artist_id": artist_uuid,
                "album_id": album_uuid,
                "artist_name": "Pink Floyd",
                "album_title": "The Wall"
            }],
            "fuzzy_threshold": 0.7,
            "auto_import_threshold": 0.8
        });
        let response = post_json(app, "/api/v1/imports/evaluate", body).await;
        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;

        assert_eq!(payload["parsed_metadata"]["artist"], "Pink Floyd");
        assert_eq!(payload["parsed_metadata"]["album"], "The Wall");
        assert_eq!(payload["parsed_metadata"]["source"], "embedded_tags");
        assert!(payload["best_match"].is_object());
        let strategy = payload["best_match"]["strategy"].as_str().unwrap();
        assert!(strategy == "exact" || strategy == "fuzzy");
        assert_eq!(payload["decision"]["decision_type"], "import");
    }

    // ---- submit_manual_import_decision ----

    #[tokio::test]
    async fn decision_happy_path_import_action() {
        let artist_uuid = "00000000-0000-0000-0000-000000000001";
        let album_uuid = "00000000-0000-0000-0000-000000000002";
        let app = imports_router();
        let body = json!({
            "action": "import",
            "artist_id": artist_uuid,
            "album_id": album_uuid
        });
        let response = post_json(app, "/api/v1/imports/decision", body).await;
        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload["accepted"], true);
        assert_eq!(payload["decision"]["decision_type"], "import");
        assert_eq!(payload["decision"]["artist_id"], artist_uuid);
        assert_eq!(payload["decision"]["album_id"], album_uuid);
    }

    #[tokio::test]
    async fn decision_normalizes_uuid_to_canonical_form() {
        let artist_uuid_upper = "00000000-0000-0000-0000-00000000ABCD";
        let album_uuid_upper = "00000000-0000-0000-0000-00000000DCBA";
        let app = imports_router();
        let body = json!({
            "action": "import",
            "artist_id": artist_uuid_upper,
            "album_id": album_uuid_upper
        });
        let response = post_json(app, "/api/v1/imports/decision", body).await;
        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        // canonical UUID strings are lowercase
        assert_eq!(
            payload["decision"]["artist_id"],
            artist_uuid_upper.to_ascii_lowercase()
        );
        assert_eq!(
            payload["decision"]["album_id"],
            album_uuid_upper.to_ascii_lowercase()
        );
    }

    #[tokio::test]
    async fn decision_happy_path_skip_action() {
        let app = imports_router();
        let body = json!({ "action": "skip", "reason": "already have it" });
        let response = post_json(app, "/api/v1/imports/decision", body).await;
        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload["accepted"], true);
        assert_eq!(payload["decision"]["decision_type"], "skip");
        assert_eq!(payload["decision"]["reason"], "already have it");
    }

    #[tokio::test]
    async fn decision_happy_path_needs_review_action() {
        let app = imports_router();
        let body = json!({ "action": "needs_review" });
        let response = post_json(app, "/api/v1/imports/decision", body).await;
        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        assert_eq!(payload["accepted"], true);
        assert_eq!(payload["decision"]["decision_type"], "needs_review");
    }

    #[tokio::test]
    async fn decision_rejects_import_without_artist_id() {
        let app = imports_router();
        let body = json!({
            "action": "import",
            "album_id": "00000000-0000-0000-0000-000000000002"
        });
        let response = post_json(app, "/api/v1/imports/decision", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert!(payload["error"].as_str().unwrap().contains("artist_id"));
    }

    #[tokio::test]
    async fn decision_rejects_import_with_invalid_artist_uuid() {
        let app = imports_router();
        let body = json!({
            "action": "import",
            "artist_id": "not-a-uuid",
            "album_id": "00000000-0000-0000-0000-000000000002"
        });
        let response = post_json(app, "/api/v1/imports/decision", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert!(payload["error"].as_str().unwrap().contains("artist_id"));
    }

    #[tokio::test]
    async fn decision_rejects_import_with_invalid_album_uuid() {
        let app = imports_router();
        let body = json!({
            "action": "import",
            "artist_id": "00000000-0000-0000-0000-000000000001",
            "album_id": "not-a-uuid"
        });
        let response = post_json(app, "/api/v1/imports/decision", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert!(payload["error"].as_str().unwrap().contains("album_id"));
    }

    #[tokio::test]
    async fn decision_rejects_import_with_confidence_out_of_range() {
        let app = imports_router();
        let body = json!({
            "action": "import",
            "artist_id": "00000000-0000-0000-0000-000000000001",
            "album_id": "00000000-0000-0000-0000-000000000002",
            "confidence": 1.5
        });
        let response = post_json(app, "/api/v1/imports/decision", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert!(payload["error"].as_str().unwrap().contains("confidence"));
    }

    #[tokio::test]
    async fn decision_rejects_unknown_action() {
        let app = imports_router();
        let body = json!({ "action": "delete_everything" });
        let response = post_json(app, "/api/v1/imports/decision", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert!(payload["error"].as_str().unwrap().contains("action"));
    }

    // ---- unit tests for helper functions ----

    #[test]
    fn map_import_decision_sets_ids() {
        let decision = ImportDecision::Import {
            artist_id: ArtistId::new(),
            album_id: AlbumId::new(),
            confidence: 0.91,
        };

        let mapped = map_decision(decision);
        assert_eq!(mapped.decision_type, "import");
        assert!(mapped.artist_id.is_some());
        assert!(mapped.album_id.is_some());
        assert_eq!(mapped.confidence, Some(0.91));
    }

    #[test]
    fn parse_artist_id_rejects_invalid_uuid() {
        let result = parse_artist_id("not-a-uuid");
        assert!(result.is_err());
    }

    #[test]
    fn parse_album_id_rejects_invalid_uuid() {
        let result = parse_album_id("not-a-uuid");
        assert!(result.is_err());
    }

    #[test]
    fn map_match_strategy_returns_stable_strings() {
        assert_eq!(map_match_strategy(&MatchStrategy::Exact), "exact");
        assert_eq!(map_match_strategy(&MatchStrategy::Fuzzy), "fuzzy");
    }

    #[test]
    fn map_metadata_source_returns_stable_strings() {
        assert_eq!(
            map_metadata_source(&MetadataSource::EmbeddedTags),
            "embedded_tags"
        );
        assert_eq!(
            map_metadata_source(&MetadataSource::FilenameHeuristics),
            "filename_heuristics"
        );
    }
}
