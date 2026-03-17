// SPDX-License-Identifier: GPL-3.0-or-later
use reqwest::Client;
use std::time::Duration;

/// Builds a `reqwest::Client` configured with the chorrosion user-agent and a 30-second timeout.
/// Falls back to a default `Client` if the builder fails.
pub(crate) fn build_http_client() -> Client {
    Client::builder()
        .user_agent(concat!(
            "chorrosion/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/SvetaKrava/chorrosion)"
        ))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|error| {
            tracing::debug!(
                target: "application",
                ?error,
                "Failed to build HTTP client with custom settings, falling back to default"
            );
            Client::new()
        })
}
