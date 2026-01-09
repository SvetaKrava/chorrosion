// SPDX-License-Identifier: GPL-3.0-or-later
use chorrosion_config::AppConfig;
pub mod events;
pub mod matching;

pub use matching::{TrackMatchingService, MatchResult, MatchingError, MatchingResult};

use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn on_start(&self) {
        info!(target: "application", "application state initialized");
    }
}
