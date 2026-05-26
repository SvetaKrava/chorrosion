# Performance baseline capture for Chorrosion.
# Measures startup readiness and request latency percentiles for key endpoints.

param(
    [int]$Iterations = 100,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

if (-not $SkipBuild) {
    cargo build --release -p chorrosion-cli
}

$exe = Join-Path $PSScriptRoot '..\target\release\chorrosion-cli.exe'
$exe = [System.IO.Path]::GetFullPath($exe)
if (-not (Test-Path $exe)) {
    throw "Binary not found: $exe"
}

$perfDir = Join-Path $PSScriptRoot '..\tmp\perf'
New-Item -ItemType Directory -Path $perfDir -Force | Out-Null

$timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$jsonOut = Join-Path $perfDir "baseline-$timestamp.json"
$latestOut = Join-Path $perfDir 'baseline-latest.json'

$env:CHORROSION_DATABASE__URL = "sqlite://data/perf-baseline-$timestamp.db"
$env:CHORROSION_AUTH__BASIC_USERNAME = 'bench'
$env:CHORROSION_AUTH__BASIC_PASSWORD = 'bench'
$env:RUST_LOG = 'warn'

$basicAuthValue = [Convert]::ToBase64String([System.Text.Encoding]::ASCII.GetBytes('bench:bench'))
$authHeaders = @{ Authorization = "Basic $basicAuthValue" }

$proc = Start-Process -FilePath $exe -PassThru
try {
    $startupSw = [System.Diagnostics.Stopwatch]::StartNew()
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    $ready = $false

    while ([DateTime]::UtcNow -lt $deadline) {
        try {
            $health = Invoke-RestMethod -Uri 'http://127.0.0.1:5150/health' -TimeoutSec 2
            if ($health.status -eq 'ok') {
                $ready = $true
                break
            }
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }

    if (-not $ready) {
        throw 'Server did not become healthy within 30s.'
    }

    $startupSw.Stop()

    $targets = @(
        @{ name = 'health'; url = 'http://127.0.0.1:5150/health'; headers = @{} },
        @{ name = 'artists_list'; url = 'http://127.0.0.1:5150/api/v1/artists?limit=50&offset=0'; headers = $authHeaders },
        @{ name = 'openapi_json'; url = 'http://127.0.0.1:5150/api-doc/openapi.json'; headers = @{} }
    )

    # Warm-up requests to avoid cold-start artifacts in latency samples.
    foreach ($target in $targets) {
        1..10 | ForEach-Object {
            $null = Invoke-WebRequest -Uri $target.url -Headers $target.headers -TimeoutSec 5 -UseBasicParsing
        }
    }

    $endpointResults = @()

    foreach ($target in $targets) {
        $samples = New-Object System.Collections.Generic.List[double]

        for ($i = 0; $i -lt $Iterations; $i++) {
            $sw = [System.Diagnostics.Stopwatch]::StartNew()
            $resp = Invoke-WebRequest -Uri $target.url -Headers $target.headers -TimeoutSec 5 -UseBasicParsing
            $sw.Stop()

            if ($resp.StatusCode -lt 200 -or $resp.StatusCode -ge 300) {
                throw "Unexpected status code $($resp.StatusCode) for $($target.name)"
            }

            $samples.Add($sw.Elapsed.TotalMilliseconds)
        }

        $sorted = $samples | Sort-Object
        $idx95 = [Math]::Min($sorted.Count - 1, [Math]::Ceiling($sorted.Count * 0.95) - 1)
        $idx50 = [Math]::Min($sorted.Count - 1, [Math]::Ceiling($sorted.Count * 0.50) - 1)

        $endpointResults += [pscustomobject]@{
            name = $target.name
            url = $target.url
            sample_count = $sorted.Count
            p50_ms = [Math]::Round($sorted[$idx50], 2)
            p95_ms = [Math]::Round($sorted[$idx95], 2)
            avg_ms = [Math]::Round(($sorted | Measure-Object -Average).Average, 2)
            max_ms = [Math]::Round(($sorted | Measure-Object -Maximum).Maximum, 2)
        }
    }

    $result = [pscustomobject]@{
        captured_at_utc = (Get-Date).ToUniversalTime().ToString('o')
        os = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
        startup_ms = [Math]::Round($startupSw.Elapsed.TotalMilliseconds, 2)
        iterations_per_endpoint = $Iterations
        endpoints = $endpointResults
    }

    $json = $result | ConvertTo-Json -Depth 5
    $json | Set-Content -Path $jsonOut -Encoding UTF8
    $json | Set-Content -Path $latestOut -Encoding UTF8

    Write-Host "Performance baseline saved: $jsonOut"
    Write-Host "Latest baseline copied to: $latestOut"
} finally {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
}
