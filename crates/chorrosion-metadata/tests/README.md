# Testing with the Mock Server

Integration tests in this crate expect a mock server to be running on `127.0.0.1:3030` before tests are executed. The test suite no longer starts or stops the mock server automatically.

## How to Run Tests

1. **Start the mock server in the background:**
   
   ```sh
   cargo run --bin mock_server &
   # or on Windows (PowerShell):
   Start-Process -NoNewWindow cargo -ArgumentList 'run --bin mock_server'
   ```

2. **Wait for the server to be ready:**
   You can use the provided helper in `tests/test_helpers.rs` to poll the server before running tests, or simply wait a few seconds.

3. **Run the tests:**
   
   ```sh
   cargo test -p chorrosion-metadata
   ```

## Helper: Wait for Mock Server

A helper function is provided in `tests/test_helpers.rs`:

```rust
// In your test (async context):
use chorrosion_metadata::test_helpers::wait_for_mock_server_ready;
wait_for_mock_server_ready("http://127.0.0.1:3030/2.0?method=ping", 10).await;
```

This will poll the server for up to 10 seconds and panic if it is not ready.

## Why?

- This approach avoids port conflicts and race conditions in CI and local runs.
- The mock server is started once for the whole test suite, not per test.

---

If you change the mock server port, update the tests and helper accordingly.
