use lidarr_config::AppConfig;
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
