// SPDX-License-Identifier: GPL-3.0-or-later
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

#[cfg(test)]
mod tests {
    use super::event_name_for_tick;

    #[test]
    fn event_name_cycles_across_supported_types() {
        assert_eq!(event_name_for_tick(0), "download_progress");
        assert_eq!(event_name_for_tick(1), "import_progress");
        assert_eq!(event_name_for_tick(2), "job_status");
        assert_eq!(event_name_for_tick(3), "download_progress");
    }
}
