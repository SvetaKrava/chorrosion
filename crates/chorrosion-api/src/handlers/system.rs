// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{extract::State, Json};
use chorrosion_application::AppState;
use serde::Serialize;
use tracing::debug;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemStatusResponse {
    pub status: &'static str,
    pub api_base: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemVersionResponse {
    pub name: &'static str,
    pub version: &'static str,
}

#[utoipa::path(
    get,
    path = "/api/v1/system/status",
    responses(
        (status = 200, description = "System status", body = SystemStatusResponse)
    ),
    tag = "system"
)]
pub async fn get_system_status(State(_state): State<AppState>) -> Json<SystemStatusResponse> {
    debug!(target: "api", "fetching system status");
    Json(SystemStatusResponse {
        status: "ok",
        api_base: "/api/v1",
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/system/version",
    responses(
        (status = 200, description = "System version", body = SystemVersionResponse)
    ),
    tag = "system"
)]
pub async fn get_system_version(State(_state): State<AppState>) -> Json<SystemVersionResponse> {
    debug!(target: "api", "fetching system version");
    Json(SystemVersionResponse {
        name: "chorrosion",
        version: env!("CARGO_PKG_VERSION"),
    })
}
