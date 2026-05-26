# Performance Baseline

This document tracks repeatable startup and API latency baselines for Chorrosion.

## How to Capture a Baseline

Windows (PowerShell):

1. Build and capture:
   powershell -ExecutionPolicy Bypass -File scripts/perf-baseline.ps1 -Iterations 100
2. Latest JSON output:
   tmp/perf/baseline-latest.json

Linux/macOS (Bash):

1. Build and capture:
   bash scripts/perf-baseline.sh 100
2. Latest JSON output:
   tmp/perf/baseline-latest.json

CI:

1. Run workflow: .github/workflows/performance-baseline.yml
2. Download artifact: performance-baseline (baseline-latest.json)

## Latest Baseline

Date (UTC): 2026-05-26T19:51:33Z
Environment: Microsoft Windows 10.0.26200
Iterations per endpoint: 100
Startup (health-ready): 584.69 ms

| Endpoint | p50 ms | p95 ms | avg ms | max ms |
| --- | ---: | ---: | ---: | ---: |
| /health | 11.33 | 12.73 | 11.40 | 14.82 |
| /api/v1/artists?limit=50&offset=0 | 11.01 | 11.79 | 11.03 | 16.26 |
| /api-doc/openapi.json | 63.20 | 89.32 | 67.90 | 149.43 |
