// SPDX-License-Identifier: GPL-3.0-or-later
use std::net::SocketAddr;

use anyhow::Result;
use axum::serve;
use chorrosion_api::router;
use chorrosion_application::{AppState, DirScanCache};
use chorrosion_config::load as load_config;
use chorrosion_infrastructure::{
    init_database,
    sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackRepository,
    },
    ResponseCache,
};
use chorrosion_scheduler::Scheduler;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = load_config(None)?;
    let pool = init_database(&config).await?;
    let artist_repository = Arc::new(SqliteArtistRepository::new_with_threshold(
        pool.clone(),
        config.database.slow_query_threshold_ms,
    ));
    let album_repository = Arc::new(SqliteAlbumRepository::new_with_threshold(
        pool.clone(),
        config.database.slow_query_threshold_ms,
    ));
    let track_repository = Arc::new(SqliteTrackRepository::new_with_threshold(
        pool.clone(),
        config.database.slow_query_threshold_ms,
    ));
    let quality_profile_repository = Arc::new(SqliteQualityProfileRepository::new(pool.clone()));
    let metadata_profile_repository = Arc::new(SqliteMetadataProfileRepository::new(pool.clone()));
    let indexer_definition_repository =
        Arc::new(SqliteIndexerDefinitionRepository::new(pool.clone()));
    let download_client_definition_repository =
        Arc::new(SqliteDownloadClientDefinitionRepository::new(pool.clone()));

    let response_cache = ResponseCache::new(
        config.cache.api_response_max_capacity,
        config.cache.api_response_ttl_seconds,
    );
    let dir_scan_cache = DirScanCache::new();

    let state = AppState::new(
        config.clone(),
        artist_repository,
        album_repository,
        track_repository,
        quality_profile_repository,
        metadata_profile_repository,
        indexer_definition_repository,
        download_client_definition_repository,
        response_cache,
        dir_scan_cache,
    );
    state.on_start();

    let scheduler = Scheduler::new(config.clone(), pool.clone());
    scheduler.register_jobs().await;
    let _scheduler_handle = scheduler.start();

    let listener = TcpListener::bind(bind_addr(&config.http)).await?;
    let addr = listener.local_addr()?;
    info!(target: "cli", "listening on {}", addr);

    serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing() {
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_names(true)
        .with_level(true);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}

fn bind_addr(http: &chorrosion_config::HttpConfig) -> SocketAddr {
    let addr = format!("{}:{}", http.host, http.port);
    addr.parse().expect("valid listen address")
}

async fn shutdown_signal() {
    #[cfg(unix)]
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("install SIGINT handler");

    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install SIGTERM handler");

    #[cfg(not(unix))]
    let interrupt = tokio::signal::ctrl_c();

    #[cfg(unix)]
    tokio::select! {
        _ = interrupt.recv() => {},
        _ = terminate.recv() => {},
    }

    #[cfg(not(unix))]
    {
        interrupt.await.expect("ctrl_c handler");
    }

    info!(target: "cli", "shutdown signal received");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bind_addr_parsing() {
        let http = chorrosion_config::HttpConfig {
            host: "127.0.0.1".to_string(),
            port: 5150,
        };
        let addr = bind_addr(&http);
        assert_eq!(addr.port(), 5150);
        assert!(addr.is_ipv4());
    }

    #[test]
    fn test_bind_addr_ipv6() {
        let http = chorrosion_config::HttpConfig {
            host: "[::1]".to_string(),
            port: 8080,
        };
        let addr = bind_addr(&http);
        assert_eq!(addr.port(), 8080);
        assert!(addr.is_ipv6());
    }

    #[cfg(unix)]
    #[test]
    fn test_unix_signal_kinds_available() {
        // Verify Unix signal kinds compile and are available
        use tokio::signal::unix::SignalKind;
        let _ = SignalKind::interrupt();
        let _ = SignalKind::terminate();
    }

    #[cfg(not(unix))]
    #[test]
    fn test_windows_signals_available() {
        // Verify Windows ctrl_c compiles
        drop(tokio::signal::ctrl_c());
    }
}
