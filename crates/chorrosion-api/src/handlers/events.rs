// SPDX-License-Identifier: GPL-3.0-or-later
use crate::handlers::activity::{
    activity_import_snapshot, activity_queue_snapshot, ActivityListResponse,
};
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use chorrosion_application::AppState;
use futures_util::stream;
use serde::Serialize;
use std::{convert::Infallible, time::Duration};
use tracing::debug;

const SSE_EVENT_INTERVAL_SECS: u64 = 5;

#[derive(Debug, Serialize)]
struct RealtimeEventPayload {
    status: &'static str,
    tick: u64,
}

#[derive(Debug, Serialize)]
struct DownloadProgressEventPayload {
    sequence: u64,
    queue: ActivityListResponse,
}

#[derive(Debug, Serialize)]
struct ImportProgressEventPayload {
    sequence: u64,
    processing: ActivityListResponse,
}

fn event_name_for_tick(tick: u64) -> &'static str {
    match tick % 3 {
        0 => "download_progress",
        1 => "import_progress",
        _ => "job_status",
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/events",
    responses(
        (status = 200, description = "Server-sent event stream for real-time updates", content_type = "text/event-stream")
    ),
    tag = "activity"
)]
pub async fn stream_events(
    State(_state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    debug!(target: "api", "opening realtime event stream");

    // Emit an initial connection event, then rotate through event types on a fixed interval.
    let events = stream::unfold((true, 0_u64), |(connected, tick)| async move {
        if connected {
            let event = Event::default()
                .event("connected")
                .data("{\"status\":\"connected\"}");
            return Some((Ok(event), (false, tick)));
        }

        tokio::time::sleep(Duration::from_secs(SSE_EVENT_INTERVAL_SECS)).await;

        let payload = RealtimeEventPayload {
            status: "idle",
            tick,
        };
        let data =
            serde_json::to_string(&payload).unwrap_or_else(|_| "{\"status\":\"idle\"}".to_string());

        let event = Event::default().event(event_name_for_tick(tick)).data(data);
        Some((Ok(event), (false, tick + 1)))
    });

    Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/events/download-progress",
    responses(
        (status = 200, description = "Server-sent download progress event stream", content_type = "text/event-stream")
    ),
    tag = "activity"
)]
pub async fn stream_download_progress_events(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    debug!(target: "api", "opening download progress event stream");

    let events = stream::unfold(
        (state, true, 0_u64),
        |(state, connected, sequence)| async move {
            if connected {
                let event = Event::default()
                    .event("connected")
                    .data("{\"status\":\"connected\"}");
                return Some((Ok(event), (state, false, sequence)));
            }

            tokio::time::sleep(Duration::from_secs(SSE_EVENT_INTERVAL_SECS)).await;

            let queue = activity_queue_snapshot(&state).await;
            let payload = DownloadProgressEventPayload { sequence, queue };
            let data = serde_json::to_string(&payload)
                .expect("DownloadProgressEventPayload is always serializable");

            let event = Event::default()
                .event("download_queue_snapshot")
                .id(sequence.to_string())
                .data(data);

            Some((Ok(event), (state, false, sequence + 1)))
        },
    );

    Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/events/import-progress",
    responses(
        (status = 200, description = "Server-sent import progress event stream", content_type = "text/event-stream")
    ),
    tag = "activity"
)]
pub async fn stream_import_progress_events(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    debug!(target: "api", "opening import progress event stream");

    let events = stream::unfold(
        (state, true, 0_u64),
        |(state, connected, sequence)| async move {
            if connected {
                let event = Event::default()
                    .event("connected")
                    .data("{\"status\":\"connected\"}");
                return Some((Ok(event), (state, false, sequence)));
            }

            tokio::time::sleep(Duration::from_secs(SSE_EVENT_INTERVAL_SECS)).await;

            let processing = activity_import_snapshot(&state).await;
            let payload = ImportProgressEventPayload {
                sequence,
                processing,
            };
            let data = serde_json::to_string(&payload)
                .expect("ImportProgressEventPayload is always serializable");

            let event = Event::default()
                .event("import_progress_snapshot")
                .id(sequence.to_string())
                .data(data);

            Some((Ok(event), (state, false, sequence + 1)))
        },
    );

    Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_name_cycles_across_supported_types() {
        assert_eq!(event_name_for_tick(0), "download_progress");
        assert_eq!(event_name_for_tick(1), "import_progress");
        assert_eq!(event_name_for_tick(2), "job_status");
        assert_eq!(event_name_for_tick(3), "download_progress");
    }

    async fn make_test_state() -> AppState {
        use chorrosion_config::AppConfig;
        use chorrosion_infrastructure::sqlite_adapters::{
            SqliteAlbumRepository, SqliteArtistRepository,
            SqliteDownloadClientDefinitionRepository, SqliteIndexerDefinitionRepository,
            SqliteMetadataProfileRepository, SqliteQualityProfileRepository, SqliteTrackRepository,
        };
        use sqlx::sqlite::SqlitePoolOptions;
        use std::sync::Arc;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations");
        AppState::new(
            AppConfig::default(),
            Arc::new(SqliteArtistRepository::new(pool.clone())),
            Arc::new(SqliteAlbumRepository::new(pool.clone())),
            Arc::new(SqliteTrackRepository::new(pool.clone())),
            Arc::new(SqliteQualityProfileRepository::new(pool.clone())),
            Arc::new(SqliteMetadataProfileRepository::new(pool.clone())),
            Arc::new(SqliteIndexerDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteDownloadClientDefinitionRepository::new(pool)),
        )
    }

    /// Collects SSE byte chunks from `stream` until a double-newline event boundary is found.
    /// Returns the accumulated text for that event.
    async fn read_next_sse_event<S, E>(stream: &mut std::pin::Pin<Box<S>>) -> String
    where
        S: futures_util::Stream<Item = Result<axum::body::Bytes, E>> + Send,
        E: std::fmt::Debug,
    {
        use futures_util::StreamExt;
        let mut buf = String::new();
        while !buf.contains("\n\n") {
            let chunk = stream
                .next()
                .await
                .expect("stream ended unexpectedly")
                .expect("stream error");
            buf.push_str(std::str::from_utf8(&chunk).expect("non-UTF-8 SSE bytes"));
        }
        buf
    }

    /// Drives the `stream_events` handler end-to-end: checks the SSE content-type header,
    /// validates the initial `connected` event, and confirms that the event name rotates
    /// deterministically across ticks.  Time is paused after state setup so the runtime
    /// auto-advances through each 5-second sleep without real waiting.
    #[tokio::test]
    async fn stream_events_content_type_initial_event_and_rotation() {
        use axum::response::IntoResponse;

        // Build state before pausing time. If time is paused first, SQLite pool
        // initialization (which uses spawn_blocking internally) may time out due to
        // Tokio's auto-advance behavior firing the pool's acquire timeout.
        let state = make_test_state().await;

        // Pause time so the runtime auto-advances when all tasks are sleeping,
        // allowing the 5-second tick intervals to complete instantly.
        tokio::time::pause();

        let sse = stream_events(State(state)).await;
        let response = sse.into_response();

        // 1. Verify SSE content-type header.
        let ct = response
            .headers()
            .get("content-type")
            .expect("missing content-type header")
            .to_str()
            .expect("non-ASCII content-type");
        assert!(
            ct.contains("text/event-stream"),
            "expected text/event-stream, got: {ct}"
        );

        // 2. Stream body as individual data chunks.
        //    Paused time causes the runtime to auto-advance when all tasks sleep,
        //    so the 5-second tick intervals complete instantly without real waiting.
        let mut data_stream = Box::pin(response.into_body().into_data_stream());

        // 3. Initial event – emitted synchronously before any sleep.
        let text = read_next_sse_event(&mut data_stream).await;
        assert!(
            text.contains("event: connected"),
            "expected connected event, got: {text}"
        );
        assert!(
            text.contains(r#"{"status":"connected"}"#),
            "expected connected payload, got: {text}"
        );

        // 4. tick 0 → download_progress  (auto-advances 5 s).
        let text = read_next_sse_event(&mut data_stream).await;
        assert!(
            text.contains("event: download_progress"),
            "expected download_progress event, got: {text}"
        );

        // 5. tick 1 → import_progress  (auto-advances another 5 s).
        let text = read_next_sse_event(&mut data_stream).await;
        assert!(
            text.contains("event: import_progress"),
            "expected import_progress event, got: {text}"
        );
    }

    #[tokio::test]
    async fn stream_download_progress_emits_download_event_with_queue_payload() {
        use axum::response::IntoResponse;

        let state = make_test_state().await;
        tokio::time::pause();

        let sse = stream_download_progress_events(State(state)).await;
        let response = sse.into_response();
        let mut data_stream = Box::pin(response.into_body().into_data_stream());

        let connected = read_next_sse_event(&mut data_stream).await;
        assert!(
            connected.contains("event: connected"),
            "expected connected event, got: {connected}"
        );

        let text = read_next_sse_event(&mut data_stream).await;
        assert!(
            text.contains("event: download_queue_snapshot"),
            "expected download_queue_snapshot event, got: {text}"
        );
        assert!(
            text.contains("\"queue\""),
            "expected queue payload, got: {text}"
        );
        assert!(
            text.contains("\"total\":0"),
            "expected empty queue total in payload, got: {text}"
        );
    }

    #[tokio::test]
    async fn stream_import_progress_emits_import_event_with_processing_payload() {
        use axum::response::IntoResponse;

        let state = make_test_state().await;
        tokio::time::pause();

        let sse = stream_import_progress_events(State(state)).await;
        let response = sse.into_response();
        let mut data_stream = Box::pin(response.into_body().into_data_stream());

        let connected = read_next_sse_event(&mut data_stream).await;
        assert!(
            connected.contains("event: connected"),
            "expected connected event, got: {connected}"
        );

        let text = read_next_sse_event(&mut data_stream).await;
        assert!(
            text.contains("event: import_progress_snapshot"),
            "expected import_progress_snapshot event, got: {text}"
        );
        assert!(
            text.contains("\"processing\""),
            "expected processing payload, got: {text}"
        );
        assert!(
            text.contains("\"total\":0"),
            "expected empty processing total in payload, got: {text}"
        );
    }
}
