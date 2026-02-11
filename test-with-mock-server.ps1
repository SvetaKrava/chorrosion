# PowerShell script to build, run mock_server, run integration tests, and clean up
# Usage: ./test-with-mock-server.ps1

# 1. Build the mock server binary and the test binary without running tests yet
cargo build -p chorrosion-metadata --bin mock_server
cargo test -p chorrosion-metadata --test lastfm_tests --no-run

# 2. Start mock_server in the background and get its process object
$mockServer = Start-Process -NoNewWindow -PassThru -FilePath "target\debug\mock_server.exe"

# 3. Wait for the mock server to be ready (poll the ping endpoint)
$maxWait = 15
$ready = $false
for ($i = 0; $i -lt $maxWait; $i++) {
    try {
        $resp = Invoke-WebRequest -Uri "http://127.0.0.1:3030/2.0?method=ping" -UseBasicParsing -TimeoutSec 1
        if ($resp.StatusCode -eq 200) {
            $ready = $true
            break
        }
    } catch {}
    Start-Sleep -Seconds 1
}
if (-not $ready) {
    Write-Error "Mock server did not become ready in $maxWait seconds."
    Stop-Process -Id $mockServer.Id
    exit 1
}

# 4. Run the compiled test binary directly to avoid rebuilding while the server is running
$testBinary = Get-ChildItem -Path "target\debug\deps" -Filter "lastfm_tests-*.exe" |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $testBinary) {
    Write-Error "Could not find lastfm_tests binary in target\\debug\\deps."
    Stop-Process -Id $mockServer.Id
    exit 1
}

& $testBinary.FullName
$testExitCode = $LASTEXITCODE

# 5. Stop the mock server
Stop-Process -Id $mockServer.Id

# 6. Exit with the test result's exit code
exit $testExitCode
