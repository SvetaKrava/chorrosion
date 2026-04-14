// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{http::StatusCode, response::IntoResponse, Json};
use chorrosion_application::{
    evaluate_import_match, parse_track_metadata, CatalogAlbum, CatalogAlbumMatch, ImportDecision,
    RawTrackMetadata,
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

    let raw = RawTrackMetadata {
        file_path: request.raw_metadata.file_path.into(),
        embedded_artist: request.raw_metadata.embedded_artist,
        embedded_album: request.raw_metadata.embedded_album,
        embedded_title: request.raw_metadata.embedded_title,
        duration_seconds: request.raw_metadata.duration_seconds,
        bitrate_kbps: request.raw_metadata.bitrate_kbps,
    };

    let parsed = parse_track_metadata(&raw)
        .await
        .map_err(|e| bad_request(&e.to_string()))?;

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
            source: format!("{:?}", parsed.source),
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
                Some(value) if !value.trim().is_empty() => value,
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
                Some(value) if !value.trim().is_empty() => value,
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

            ImportDecisionResponse {
                decision_type: "import".to_string(),
                artist_id: Some(artist_id),
                album_id: Some(album_id),
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

fn map_best_match(best_match: CatalogAlbumMatch) -> CatalogAlbumMatchResponse {
    CatalogAlbumMatchResponse {
        artist_id: best_match.artist_id.to_string(),
        album_id: best_match.album_id.to_string(),
        confidence: best_match.confidence,
        strategy: format!("{:?}", best_match.strategy),
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
}
