use tokio::signal;
use warp::Filter;
use serde_json::json;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    let port = 3030;
    println!("Starting mock server on http://127.0.0.1:{}", port);

    // Define routes
    let artist_info = warp::path("2.0")
        .and(warp::query::<HashMap<String, String>>())
        .map(|params: HashMap<String, String>| {
            let default_artist = "Unknown Artist".to_string();
            if let Some(method) = params.get("method") {
                match method.as_str() {
                    "artist.getinfo" => {
                        let artist = params.get("artist").unwrap_or(&default_artist);
                        warp::reply::json(&json!({
                            "artist": {
                                "title": artist,
                                "artist": artist,
                                "tracks": ["Track 1", "Track 2", "Track 3"]
                            }
                        }))
                    },
                    _ => warp::reply::json(&json!({ "error": "Unknown method" })),
                }
            } else {
                warp::reply::json(&json!({ "error": "Missing method parameter" }))
            }
        });

    let routes = artist_info;

    // Start the server
    let (_, server) = warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], port), async {
        signal::ctrl_c().await.expect("Failed to install CTRL+C signal handler");
        println!("Received termination signal. Shutting down mock server...");
    });

    println!("Mock server is running on port {}", port);
    server.await;
    println!("Mock server has stopped running.");
}