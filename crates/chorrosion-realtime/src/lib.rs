// SPDX-License-Identifier: GPL-3.0-or-later
use tracing::info;

#[async_trait::async_trait]
pub trait RealtimeHub: Send + Sync + 'static {
    async fn broadcast(&self, channel: &str, payload: &str);
}

pub struct NoopRealtimeHub;

#[async_trait::async_trait]
impl RealtimeHub for NoopRealtimeHub {
    async fn broadcast(&self, channel: &str, payload: &str) {
        info!(target: "realtime", %channel, %payload, "noop realtime broadcast");
    }
}
