#!/usr/bin/env bash
set -euo pipefail

ITERATIONS="${1:-100}"
if ! [[ "$ITERATIONS" =~ ^[0-9]+$ ]]; then
  echo "iterations must be numeric" >&2
  exit 1
fi

cargo build --release -p chorrosion-cli

mkdir -p tmp/perf data
timestamp="$(date -u +%Y%m%d-%H%M%S)"
json_out="tmp/perf/baseline-${timestamp}.json"
latest_out="tmp/perf/baseline-latest.json"

export CHORROSION_DATABASE__URL="sqlite://data/perf-baseline-${timestamp}.db"
export CHORROSION_AUTH__BASIC_USERNAME="bench"
export CHORROSION_AUTH__BASIC_PASSWORD="bench"
export RUST_LOG="warn"

auth_b64="$(printf 'bench:bench' | base64 | tr -d '\n')"
auth_header="Authorization: Basic ${auth_b64}"

target/release/chorrosion-cli > /dev/null 2>&1 &
server_pid=$!

cleanup() {
  kill "$server_pid" >/dev/null 2>&1 || true
  wait "$server_pid" >/dev/null 2>&1 || true
}
trap cleanup EXIT

startup_start_ms="$(date +%s%3N)"
ready="false"
for _ in $(seq 1 300); do
  if curl -fsS "http://127.0.0.1:5150/health" >/dev/null 2>&1; then
    ready="true"
    break
  fi
  sleep 0.1
done
if [[ "$ready" != "true" ]]; then
  echo "server did not become healthy within 30s" >&2
  exit 1
fi
startup_end_ms="$(date +%s%3N)"
startup_ms="$((startup_end_ms - startup_start_ms))"

endpoints=(
  "health|http://127.0.0.1:5150/health|"
  "artists_list|http://127.0.0.1:5150/api/v1/artists?limit=50&offset=0|${auth_header}"
  "openapi_json|http://127.0.0.1:5150/api-doc/openapi.json|"
)

json_endpoints="[]"

for endpoint in "${endpoints[@]}"; do
  name="${endpoint%%|*}"
  rest="${endpoint#*|}"
  url="${rest%%|*}"
  header="${rest#*|}"

  for _ in $(seq 1 10); do
    if [[ -n "$header" ]]; then
      curl -fsS -H "$header" "$url" >/dev/null
    else
      curl -fsS "$url" >/dev/null
    fi
  done

  samples=()
  for _ in $(seq 1 "$ITERATIONS"); do
    if [[ -n "$header" ]]; then
      val="$(curl -s -H "$header" -o /dev/null -w '%{time_total}' "$url")"
    else
      val="$(curl -s -o /dev/null -w '%{time_total}' "$url")"
    fi
    ms="$(awk -v t="$val" 'BEGIN { printf "%.3f", t * 1000 }')"
    samples+=("$ms")
  done

  sorted="$(printf '%s
' "${samples[@]}" | sort -n)"
  p50_index="$(( (ITERATIONS + 1) / 2 ))"
  p95_index="$(( (ITERATIONS * 95 + 99) / 100 ))"
  if (( p95_index > ITERATIONS )); then
    p95_index="$ITERATIONS"
  fi

  p50="$(printf '%s
' "$sorted" | sed -n "${p50_index}p")"
  p95="$(printf '%s
' "$sorted" | sed -n "${p95_index}p")"
  avg="$(printf '%s
' "${samples[@]}" | awk '{sum += $1} END {if (NR == 0) print "0.000"; else printf "%.3f", sum / NR}')"
  max="$(printf '%s
' "$sorted" | tail -n 1)"

  endpoint_json="$(jq -n \
    --arg name "$name" \
    --arg url "$url" \
    --argjson sample_count "$ITERATIONS" \
    --argjson p50_ms "$p50" \
    --argjson p95_ms "$p95" \
    --argjson avg_ms "$avg" \
    --argjson max_ms "$max" \
    '{name:$name,url:$url,sample_count:$sample_count,p50_ms:$p50_ms,p95_ms:$p95_ms,avg_ms:$avg_ms,max_ms:$max_ms}')"

  json_endpoints="$(jq --argjson obj "$endpoint_json" '. + [$obj]' <<< "$json_endpoints")"
done

jq -n \
  --arg captured_at_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg os "$(uname -srm)" \
  --argjson startup_ms "$startup_ms" \
  --argjson iterations_per_endpoint "$ITERATIONS" \
  --argjson endpoints "$json_endpoints" \
  '{captured_at_utc:$captured_at_utc,os:$os,startup_ms:$startup_ms,iterations_per_endpoint:$iterations_per_endpoint,endpoints:$endpoints}' \
  > "$json_out"

cp "$json_out" "$latest_out"
echo "Performance baseline saved: $json_out"
echo "Latest baseline copied to: $latest_out"
