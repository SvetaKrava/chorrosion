// SPDX-License-Identifier: GPL-3.0-or-later
use chorrosion_config::AppConfig;
pub mod embedded_tags;
pub mod events;
pub mod import;
pub mod matching;

pub use embedded_tags::{EmbeddedTagError, EmbeddedTagMatchingService, EmbeddedTagResult};
pub use import::{FileImportService, ImportError, ImportResult, ImportedFile};
pub use matching::{MatchResult, MatchingError, MatchingResult, TrackMatchingService};

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
