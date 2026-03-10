// SPDX-License-Identifier: GPL-3.0-or-later
use crate::handlers::activity::{
    activity_import_snapshot, activity_queue_snapshot, ActivityListResponse,
};
use crate::handlers::system::{system_tasks_snapshot, SystemTasksResponse};
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use chorrosion_application::AppState;
use futures_util::stream;
use serde::Serialize;
use std::{
    convert::Infallible,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use tracing::debug;
use utoipa::ToSchema;

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

#[derive(Debug, Serialize)]
struct JobStatusEventPayload {
    sequence: u64,
    tasks: SystemTasksResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SseConnectionsResponse {
    pub total: usize,
    pub events: usize,
    pub download_progress: usize,
    pub import_progress: usize,
    pub job_status: usize,
}

#[derive(Clone, Copy)]
enum StreamKind {
    Events,
    DownloadProgress,
    ImportProgress,
    JobStatus,
}

static EVENTS_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
static DOWNLOAD_PROGRESS_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
static IMPORT_PROGRESS_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
static JOB_STATUS_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

fn counter_for(kind: StreamKind) -> &'static AtomicUsize {
    match kind {
        StreamKind::Events => &EVENTS_CONNECTIONS,
        StreamKind::DownloadProgress => &DOWNLOAD_PROGRESS_CONNECTIONS,
        StreamKind::ImportProgress => &IMPORT_PROGRESS_CONNECTIONS,
        StreamKind::JobStatus => &JOB_STATUS_CONNECTIONS,
    }
}

fn sse_connections_snapshot() -> SseConnectionsResponse {
    let events = EVENTS_CONNECTIONS.load(Ordering::Relaxed);
    let download_progress = DOWNLOAD_PROGRESS_CONNECTIONS.load(Ordering::Relaxed);
    let import_progress = IMPORT_PROGRESS_CONNECTIONS.load(Ordering::Relaxed);
    let job_status = JOB_STATUS_CONNECTIONS.load(Ordering::Relaxed);

    SseConnectionsResponse {
        total: events + download_progress + import_progress + job_status,
        events,
        download_progress,
        import_progress,
        job_status,
    }
}

struct ConnectionGuard {
    kind: StreamKind,
}

impl ConnectionGuard {
    fn new(kind: StreamKind) -> Self {
        counter_for(kind).fetch_add(1, Ordering::Relaxed);
        Self { kind }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        counter_for(self.kind).fetch_sub(1, Ordering::Relaxed);
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/events/connections",
    responses(
        (status = 200, description = "Current SSE connection counts", body = SseConnectionsResponse)
    ),
    tag = "activity"
)]
pub async fn get_sse_connections() -> Json<SseConnectionsResponse> {
    Json(sse_connections_snapshot())
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
    let events = stream::unfold(
        (true, 0_u64, ConnectionGuard::new(StreamKind::Events)),
        |(connected, tick, guard)| async move {
            if connected {
                let event = Event::default()
                    .event("connected")
                    .data("{\"status\":\"connected\"}");
                return Some((Ok(event), (false, tick, guard)));
            }

            tokio::time::sleep(Duration::from_secs(SSE_EVENT_INTERVAL_SECS)).await;

            let payload = RealtimeEventPayload {
                status: "idle",
                tick,
            };
            let data = serde_json::to_string(&payload)
                .unwrap_or_else(|_| "{\"status\":\"idle\"}".to_string());

            let event = Event::default().event(event_name_for_tick(tick)).data(data);
            Some((Ok(event), (false, tick + 1, guard)))
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
        (
            state,
            true,
            0_u64,
            ConnectionGuard::new(StreamKind::DownloadProgress),
        ),
        |(state, connected, sequence, guard)| async move {
            if connected {
                let event = Event::default()
                    .event("connected")
                    .data("{\"status\":\"connected\"}");
                return Some((Ok(event), (state, false, sequence, guard)));
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

            Some((Ok(event), (state, false, sequence + 1, guard)))
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
        (
            state,
            true,
            0_u64,
            ConnectionGuard::new(StreamKind::ImportProgress),
        ),
        |(state, connected, sequence, guard)| async move {
            if connected {
                let event = Event::default()
                    .event("connected")
                    .data("{\"status\":\"connected\"}");
                return Some((Ok(event), (state, false, sequence, guard)));
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

            Some((Ok(event), (state, false, sequence + 1, guard)))
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
    path = "/api/v1/events/job-status",
    responses(
        (status = 200, description = "Server-sent job status event stream", content_type = "text/event-stream")
    ),
    tag = "activity"
)]
pub async fn stream_job_status_events(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    debug!(target: "api", "opening job status event stream");

    let events = stream::unfold(
        (
            state,
            true,
            0_u64,
            ConnectionGuard::new(StreamKind::JobStatus),
        ),
        |(state, connected, sequence, guard)| async move {
            if connected {
                let event = Event::default()
                    .event("connected")
                    .data("{\"status\":\"connected\"}");
                return Some((Ok(event), (state, false, sequence, guard)));
            }

            tokio::time::sleep(Duration::from_secs(SSE_EVENT_INTERVAL_SECS)).await;

            let tasks = system_tasks_snapshot(&state).await;
            let payload = JobStatusEventPayload { sequence, tasks };
            let data = serde_json::to_string(&payload)
                .expect("JobStatusEventPayload is always serializable");

            let event = Event::default()
                .event("job_status_snapshot")
                .id(sequence.to_string())
                .data(data);

            Some((Ok(event), (state, false, sequence + 1, guard)))
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

    #[tokio::test]
    async fn stream_job_status_emits_job_status_event_with_tasks_payload() {
        use axum::response::IntoResponse;

        let state = make_test_state().await;
        tokio::time::pause();

        let sse = stream_job_status_events(State(state)).await;
        let response = sse.into_response();
        let mut data_stream = Box::pin(response.into_body().into_data_stream());

        let connected = read_next_sse_event(&mut data_stream).await;
        assert!(
            connected.contains("event: connected"),
            "expected connected event, got: {connected}"
        );

        let text = read_next_sse_event(&mut data_stream).await;
        assert!(
            text.contains("event: job_status_snapshot"),
            "expected job_status_snapshot event, got: {text}"
        );
        assert!(
            text.contains("\"tasks\""),
            "expected tasks payload, got: {text}"
        );
        assert!(
            text.contains("\"rss-sync\""),
            "expected rss-sync job in payload, got: {text}"
        );
    }

    #[tokio::test]
    async fn get_sse_connections_tracks_event_connection_lifecycle() {
        // Take an initial snapshot of the SSE connection counts.
        let Json(initial) = get_sse_connections().await;

        {
            // Create a ConnectionGuard for an Events stream and verify it increments counts.
            let _guard = super::ConnectionGuard::new(super::StreamKind::Events);

            let Json(with_event) = get_sse_connections().await;

            assert_eq!(
                with_event.events,
                initial.events + 1,
                "expected events count to increase by 1 while ConnectionGuard is held"
            );
            assert_eq!(
                with_event.total,
                initial.total + 1,
                "expected total count to increase by 1 while ConnectionGuard is held"
            );
        }

        // After the guard is dropped, counts should return to their initial values.
        let Json(after) = get_sse_connections().await;

        assert_eq!(
            after.events, initial.events,
            "expected events count to return to initial value after ConnectionGuard is dropped"
        );
        assert_eq!(
            after.total, initial.total,
            "expected total count to return to initial value after ConnectionGuard is dropped"
        );
    }
}
