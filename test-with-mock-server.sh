#!/usr/bin/env bash
# test-with-mock-server.sh: Build, run mock_server, run integration tests, and clean up (Linux/macOS)
set -euo pipefail

# 1. Build the mock server binary and the test binary without running tests yet
cargo build -p chorrosion-metadata --bin mock_server
cargo test -p chorrosion-metadata --test lastfm_tests --no-run

# 2. Start mock_server in the background
nohup target/debug/mock_server > mock_server.log 2>&1 &
MOCK_SERVER_PID=$!

# Set trap to clean up mock_server on any exit
trap 'kill $MOCK_SERVER_PID 2>/dev/null || true; wait $MOCK_SERVER_PID 2>/dev/null || true' EXIT


# 3. Wait for the mock server to be ready (poll the ping endpoint)
MAX_WAIT=15
READY=0
for i in $(seq 1 $MAX_WAIT); do
    if curl -s http://127.0.0.1:3030/2.0?method=ping > /dev/null; then
        READY=1
        break
    fi
    sleep 1
done
if [ $READY -ne 1 ]; then
    echo "Mock server did not become ready in $MAX_WAIT seconds." >&2
    kill $MOCK_SERVER_PID || true
    exit 1
fi

# 4. Run the compiled test binary directly to avoid rebuilding while the server is running
TEST_BINARY=$(ls -t target/debug/deps/lastfm_tests-* 2>/dev/null | head -n 1)
if [ -z "$TEST_BINARY" ]; then
    echo "Could not find lastfm_tests binary in target/debug/deps." >&2
    kill $MOCK_SERVER_PID || true
    exit 1
fi

"$TEST_BINARY" --include-ignored
TEST_RESULT=$?

# 5. Stop the mock server
kill $MOCK_SERVER_PID || true
wait $MOCK_SERVER_PID 2>/dev/null || true

# 6. Exit with the test result's exit code
exit $TEST_RESULT
