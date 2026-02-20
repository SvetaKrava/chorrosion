use tokio::signal;
use warp::Filter;
use serde_json::json;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    let port = 3030;
    println!("Starting mock server on http://127.0.0.1:{}", port);

    // Define routes
    let api = warp::path::full()
        .and(warp::query::<HashMap<String, String>>())
        .map(|_path: warp::path::FullPath, params: HashMap<String, String>| {
            let default_artist = "Unknown Artist".to_string();
            let default_album = "Unknown Album".to_string();

            match params.get("method").map(String::as_str) {
                Some("artist.getinfo") => {
                    let artist = params.get("artist").unwrap_or(&default_artist);
                    warp::reply::json(&json!({
                        "name": artist,
                        "bio": "Test artist bio",
                        "tags": ["rock", "indie"]
                    }))
                }
                Some("album.getinfo") => {
                    let artist = params.get("artist").unwrap_or(&default_artist);
                    let album = params.get("album").unwrap_or(&default_album);
                    warp::reply::json(&json!({
                        "title": album,
                        "artist": artist,
                        "tracks": ["Track 1", "Track 2", "Track 3"]
                    }))
                }
                Some(_) => warp::reply::json(&json!({ "error": "Unknown method" })),
                None => warp::reply::json(&json!({ "error": "Missing method parameter" })),
            }
        });

    let routes = api;

    // Start the server
    let (_, server) = warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], port), async {
        signal::ctrl_c().await.expect("Failed to install CTRL+C signal handler");
        println!("Received termination signal. Shutting down mock server...");
    });

    println!("Mock server is running on port {}", port);
    server.await;
    println!("Mock server has stopped running.");
}