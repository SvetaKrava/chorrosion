// SPDX-License-Identifier: GPL-3.0-or-later
use crate::handlers::activity::{
    activity_import_snapshot, activity_queue_snapshot, ActivityListResponse,
};
use crate::handlers::system::{system_tasks_snapshot, SystemTasksResponse};
use axum::{
    extract::State,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use chorrosion_application::AppState;
use futures_util::stream;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    sync::atomic::{AtomicUsize, Ordering},
    sync::OnceLock,
    time::Duration,
};
use tokio::sync::broadcast;
use tracing::{debug, warn};
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

#[derive(Debug, Clone)]
struct BroadcastEvent {
    event: String,
    payload: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BroadcastEventRequest {
    pub event: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BroadcastEventResponse {
    pub accepted: bool,
    pub delivered_to: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BroadcastErrorResponse {
    pub error: String,
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
static EVENT_BROADCASTER: OnceLock<broadcast::Sender<BroadcastEvent>> = OnceLock::new();

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

fn event_broadcaster() -> &'static broadcast::Sender<BroadcastEvent> {
    EVENT_BROADCASTER.get_or_init(|| {
        let (sender, _receiver) = broadcast::channel(256);
        sender
    })
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

#[utoipa::path(
    post,
    path = "/api/v1/events/broadcast",
    request_body = BroadcastEventRequest,
    responses(
        (status = 202, description = "Broadcast event accepted", body = BroadcastEventResponse),
        (status = 400, description = "Invalid broadcast event payload", body = BroadcastErrorResponse)
    ),
    tag = "activity"
)]
pub async fn post_broadcast_event(
    Json(request): Json<BroadcastEventRequest>,
) -> Result<(StatusCode, Json<BroadcastEventResponse>), (StatusCode, Json<BroadcastErrorResponse>)>
{
    let event = request.event.trim();
    if event.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BroadcastErrorResponse {
                error: "event name must not be empty".to_string(),
            }),
        ));
    }
    if event.contains(char::is_control) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BroadcastErrorResponse {
                error: "event name must not contain control characters".to_string(),
            }),
        ));
    }

    let payload = request.payload.to_string();
    let delivered_to = event_broadcaster()
        .send(BroadcastEvent {
            event: event.to_string(),
            payload,
        })
        .unwrap_or(0);

    Ok((
        StatusCode::ACCEPTED,
        Json(BroadcastEventResponse {
            accepted: true,
            delivered_to,
        }),
    ))
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

    let receiver = event_broadcaster().subscribe();

    // Emit an initial connection event, then rotate through event types on a fixed interval.
    let events = stream::unfold(
        (
            true,
            0_u64,
            ConnectionGuard::new(StreamKind::Events),
            receiver,
        ),
        |(connected, tick, guard, mut receiver)| async move {
            if connected {
                let event = Event::default()
                    .event("connected")
                    .data("{\"status\":\"connected\"}");
                return Some((Ok(event), (false, tick, guard, receiver)));
            }

            let event = tokio::select! {
                recv_result = receiver.recv() => {
                    match recv_result {
                        Ok(message) => Event::default().event(message.event).data(message.payload),
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            Event::default().event("broadcast_lagged").data("{\"status\":\"lagged\"}")
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            Event::default().event("broadcast_closed").data("{\"status\":\"closed\"}")
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(SSE_EVENT_INTERVAL_SECS)) => {
                    let payload = RealtimeEventPayload {
                        status: "idle",
                        tick,
                    };
                    let data = serde_json::to_string(&payload)
                        .unwrap_or_else(|_| "{\"status\":\"idle\"}".to_string());
                    Event::default().event(event_name_for_tick(tick)).data(data)
                }
            };

            Some((Ok(event), (false, tick + 1, guard, receiver)))
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

            let queue = activity_queue_snapshot(&state).await.unwrap_or_else(|e| {
                warn!(target: "api", error = %e, "SSE: failed to build activity queue snapshot");
                ActivityListResponse {
                    items: vec![],
                    total: 0,
                }
            });
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
    use std::sync::OnceLock;

    /// Global mutex serializing all tests that read or write the global SSE connection counters.
    /// Rust tests run in parallel by default, and several tests in this module create SSE streams
    /// (each of which increments a counter via `ConnectionGuard`). Without serialization the
    /// lifecycle test's exact +1 / return-to-initial assertions would be non-deterministic.
    static COUNTER_TEST_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

    fn counter_test_mutex() -> &'static tokio::sync::Mutex<()> {
        COUNTER_TEST_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
    }

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
            SqliteMetadataProfileRepository, SqliteQualityProfileRepository, SqliteTagRepository,
            SqliteTaggedEntityRepository, SqliteTrackRepository,
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
            Arc::new(SqliteDownloadClientDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteTagRepository::new(pool.clone())),
            Arc::new(SqliteTaggedEntityRepository::new(pool.clone())),
            Arc::new(
                chorrosion_infrastructure::sqlite_adapters::SqliteSmartPlaylistRepository::new(
                    pool.clone(),
                ),
            ),
            Arc::new(
                chorrosion_infrastructure::sqlite_adapters::SqliteDuplicateRepository::new(
                    pool.clone(),
                ),
            ),
            chorrosion_infrastructure::ResponseCache::new(100, 60),
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

    /// Like [`read_next_sse_event`] but skips SSE keepalive comments.
    ///
    /// With `tokio::time::pause()`, keepalive timers can race with real async
    /// I/O (e.g. SQLite queries inside `activity_queue_snapshot`) causing a
    /// keepalive to arrive before the expected data event.  The function skips
    /// up to 50 consecutive keepalive frames to avoid hanging indefinitely.
    async fn read_next_data_event<S, E>(stream: &mut std::pin::Pin<Box<S>>) -> String
    where
        S: futures_util::Stream<Item = Result<axum::body::Bytes, E>> + Send,
        E: std::fmt::Debug,
    {
        const MAX_KEEPALIVES: usize = 50;
        for _ in 0..MAX_KEEPALIVES {
            let text = read_next_sse_event(stream).await;
            if !text.trim().starts_with(": keepalive") {
                return text;
            }
        }
        panic!(
            "exceeded {MAX_KEEPALIVES} consecutive keepalive frames without receiving a data event"
        );
    }

    /// Drives the `stream_events` handler end-to-end: checks the SSE content-type header,
    /// validates the initial `connected` event, and confirms that the event name rotates
    /// deterministically across ticks.  Time is paused after state setup so the runtime
    /// auto-advances through each 5-second sleep without real waiting.
    #[tokio::test]
    async fn stream_events_content_type_initial_event_and_rotation() {
        use axum::response::IntoResponse;
        let _lock = counter_test_mutex().lock().await;

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
        let _lock = counter_test_mutex().lock().await;

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

        let text = read_next_data_event(&mut data_stream).await;
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
        let _lock = counter_test_mutex().lock().await;

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
        let _lock = counter_test_mutex().lock().await;

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
        let _lock = counter_test_mutex().lock().await;

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

    #[tokio::test]
    async fn post_broadcast_event_rejects_empty_event_name() {
        let result = post_broadcast_event(Json(BroadcastEventRequest {
            event: "   ".to_string(),
            payload: serde_json::json!({}),
        }))
        .await;

        assert!(matches!(result, Err((StatusCode::BAD_REQUEST, _))));
        let Err((_, Json(err))) = result else {
            panic!("expected error")
        };
        assert!(!err.error.is_empty(), "error body should contain a message");
    }

    #[tokio::test]
    async fn post_broadcast_event_rejects_event_name_with_newline() {
        let result = post_broadcast_event(Json(BroadcastEventRequest {
            event: "bad\nevent".to_string(),
            payload: serde_json::json!({}),
        }))
        .await;

        assert!(matches!(result, Err((StatusCode::BAD_REQUEST, _))));
        let Err((_, Json(err))) = result else {
            panic!("expected error")
        };
        assert!(
            err.error.contains("control characters"),
            "expected control character error, got: {}",
            err.error
        );
    }

    #[tokio::test]
    async fn post_broadcast_event_rejects_event_name_with_carriage_return() {
        let result = post_broadcast_event(Json(BroadcastEventRequest {
            event: "bad\revent".to_string(),
            payload: serde_json::json!({}),
        }))
        .await;

        assert!(matches!(result, Err((StatusCode::BAD_REQUEST, _))));
        let Err((_, Json(err))) = result else {
            panic!("expected error")
        };
        assert!(
            err.error.contains("control characters"),
            "expected control character error, got: {}",
            err.error
        );
    }

    #[tokio::test]
    async fn post_broadcast_event_rejects_event_name_with_other_control_char() {
        let result = post_broadcast_event(Json(BroadcastEventRequest {
            event: "bad\x00event".to_string(),
            payload: serde_json::json!({}),
        }))
        .await;

        assert!(matches!(result, Err((StatusCode::BAD_REQUEST, _))));
        let Err((_, Json(err))) = result else {
            panic!("expected error")
        };
        assert!(
            err.error.contains("control characters"),
            "expected control character error, got: {}",
            err.error
        );
    }

    #[tokio::test]
    async fn stream_events_receives_custom_broadcast_event() {
        use axum::response::IntoResponse;
        let _lock = counter_test_mutex().lock().await;

        let state = make_test_state().await;
        let sse = stream_events(State(state)).await;
        let response = sse.into_response();
        let mut data_stream = Box::pin(response.into_body().into_data_stream());

        let connected = read_next_sse_event(&mut data_stream).await;
        assert!(connected.contains("event: connected"));

        let publish = post_broadcast_event(Json(BroadcastEventRequest {
            event: "custom_broadcast".to_string(),
            payload: serde_json::json!({"kind": "test"}),
        }))
        .await
        .expect("broadcast should be accepted");

        assert_eq!(publish.0, StatusCode::ACCEPTED);

        let text = read_next_sse_event(&mut data_stream).await;
        assert!(
            text.contains("event: custom_broadcast"),
            "expected custom broadcast event, got: {text}"
        );
        assert!(
            text.contains("{\"kind\":\"test\"}"),
            "expected custom payload, got: {text}"
        );
    }
}
