# Chorrosion

[![CI](https://github.com/SvetaKrava/chorrosion/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/SvetaKrava/chorrosion/actions/workflows/ci.yml)
[![Coverage](https://github.com/SvetaKrava/chorrosion/actions/workflows/coverage.yml/badge.svg?branch=main)](https://github.com/SvetaKrava/chorrosion/actions/workflows/coverage.yml)
[![Security](https://github.com/SvetaKrava/chorrosion/actions/workflows/security.yml/badge.svg?branch=main)](https://github.com/SvetaKrava/chorrosion/actions/workflows/security.yml)
[![Release](https://github.com/SvetaKrava/chorrosion/actions/workflows/release.yml/badge.svg)](https://github.com/SvetaKrava/chorrosion/actions/workflows/release.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)

Axum + Tokio powered service with a typed domain model, OpenAPI-documented REST API, and a lightweight job scheduler. Built as a Rust workspace for clear separation of concerns and cross-platform reliability (Windows, Linux, macOS).

## Features

- REST API with OpenAPI/Swagger UI at `/docs` (via `utoipa`).
- Health endpoint at `/health`.
- Config layering with Figment: sensible defaults → optional TOML → environment (`CHORROSION_` prefix with `__` for nesting).
- SQLx with SQLite by default; migrations applied on startup from `./migrations`.
- Tokio-based scheduler with job registry, intervals, retry behavior, and concurrency limits.
- Strongly-typed domain IDs wrapping `Uuid` with string serialization for APIs.
- Cross-platform: consistent behavior across Windows, Linux, and macOS.

## Quickstart

### Prerequisites

- Rust toolchain (stable) — see [rustup.rs](https://rustup.rs/)
- `cargo`
- **System libraries**: Chromaprint and FFmpeg for audio fingerprinting — see [EXTERNAL_DEPENDENCIES.md](EXTERNAL_DEPENDENCIES.md) for platform-specific installation

Default server binds to `127.0.0.1:5150`.

### Windows (PowerShell)

```powershell
# Optional: increase log verbosity
$env:RUST_LOG = "info,api=debug,registry=debug"

# Optional: set SQLite location (directory created on first run)
$env:CHORROSION_DATABASE__URL = "sqlite://data/chorrosion.db"

# Run CLI (applies migrations, starts scheduler, serves API)
cargo run -p chorrosion-cli
```

### Bash/Zsh

```bash
# Optional: increase log verbosity
RUST_LOG=info,api=debug,registry=debug \
CHORROSION_DATABASE__URL=sqlite://data/chorrosion.db \
cargo run -p chorrosion-cli
```

Once running:

- Swagger UI: <http://127.0.0.1:5150/docs>
- Health check: <http://127.0.0.1:5150/health>

## Web UI (Phase 11)

The repository now includes a SvelteKit frontend in `web/` with the initial
control surface for:

- Forms login/logout
- Realtime dashboard cards (SSE-backed queue/import/job status)
- Appearance settings editor (`/api/v1/settings/appearance`)

### Frontend Development (Bun)

```powershell
cd web
bun install
bun run dev
```

Optional frontend env file:

```text
VITE_CHORROSION_API_BASE=http://127.0.0.1:5150
```

### Backend Web Config (Env)

- `CHORROSION_WEB__ALLOWED_ORIGINS` (comma-separated origins)
- `CHORROSION_WEB__SERVE_STATIC_ASSETS` (`true`/`false`)
- `CHORROSION_WEB__STATIC_DIST_DIR` (default: `web/build`)
- `CHORROSION_AUTH__FORMS_COOKIE_SECURE` (`true` in prod, `false` for localhost HTTP)

Example local dev setup:

```powershell
$env:CHORROSION_WEB__ALLOWED_ORIGINS="http://127.0.0.1:5173,http://localhost:5173"
$env:CHORROSION_AUTH__FORMS_COOKIE_SECURE="false"
cargo run -p chorrosion-cli
```

### Production Static Serving

Build the frontend and let Axum serve the generated static files:

```powershell
cd web
bun run build
cd ..
$env:CHORROSION_WEB__SERVE_STATIC_ASSETS="true"
$env:CHORROSION_WEB__STATIC_DIST_DIR="web/build"
cargo run -p chorrosion-cli
```

## Releases

- Tagged releases (`vX.Y.Z`) automatically build cross-platform archives.
- See the Releases tab for downloadable artifacts. To create a release:
  - Create and push a tag, e.g. `git tag v0.1.0 && git push origin v0.1.0`.

## Configuration

1. Code defaults
2. Optional TOML file (wiring from CLI planned)
3. Environment variables with `CHORROSION_` prefix and `__` for nesting

- `CHORROSION_DATABASE__URL=sqlite://data/chorrosion.db`
- `CHORROSION_HTTP__HOST=127.0.0.1`
- `CHORROSION_SCHEDULER__MAX_CONCURRENT_JOBS=4`
- `CHORROSION_WEB__ALLOWED_ORIGINS=http://127.0.0.1:5173,http://localhost:5173`

PostgreSQL-only pool tuning settings:

> These pool tuning env vars currently apply when using the PostgreSQL backend.
> The default SQLite pool ignores these settings.

- `CHORROSION_DATABASE__POOL_MAX_SIZE=16`
- `CHORROSION_DATABASE__POOL_MIN_CONNECTIONS=1`
- `CHORROSION_DATABASE__POOL_ACQUIRE_TIMEOUT_SECS=10`
- `CHORROSION_DATABASE__POOL_IDLE_TIMEOUT_SECS=600`
- `CHORROSION_DATABASE__POOL_MAX_LIFETIME_SECS=1800`

SQLite tips:

- Use `?mode=rwc` in URLs if you need create-on-open semantics.

### PostgreSQL Feature Gate

PostgreSQL support is intentionally gated behind an opt-in Cargo feature and is
disabled by default in workspace builds.

- Default (`cargo build --workspace`): SQLite-only dependency graph
- Opt-in PostgreSQL build path:

```bash
cargo build -p chorrosion-cli --features postgres
```

## API

- Base: `/api/v1`
- OpenAPI docs are generated with `utoipa` and exposed at `/docs`.

### Authentication

All `/api/v1` endpoints require a valid API key. Provide the key using one of:

- **Header:** `X-Api-Key: <key>`
- **Bearer token:** `Authorization: Bearer <key>`

#### HTTP Basic auth (optional)

When both `CHORROSION_AUTH__BASIC_USERNAME` and `CHORROSION_AUTH__BASIC_PASSWORD` are set, HTTP Basic authentication is also accepted:

```
Authorization: Basic <base64(username:password)>
```

When Basic auth is configured, API key auth remains supported alongside it.

#### Bootstrap flow (first-time setup)

When no API keys exist, `POST /api/v1/auth/api-keys` is accessible without authentication so the first key can be created:

```bash
curl -X POST http://localhost:5150/api/v1/auth/api-keys \
  -H 'Content-Type: application/json' \
  -d '{"name": "my-key"}'
# → { "id": "...", "key": "ck_...", ... }
```

While the process is running and at least one key exists, all subsequent requests (including key management) require a valid key.

> **Security note:** API keys are currently kept in an in-memory store. On each process restart the store is cleared, so the service briefly reverts to a zero-key state and the bootstrap route becomes unauthenticated again until a new key is created. Do **not** expose the service directly to the internet; keep it bound to localhost or behind a trusted reverse proxy and create a new API key immediately after each restart.

Current implemented endpoints:

- `GET /health`
- `GET /api/v1/auth/api-keys`
- `POST /api/v1/auth/api-keys`
- `DELETE /api/v1/auth/api-keys/{id}`
- `GET /api/v1/artists`
- `GET /api/v1/artists/{id}`
- `GET /api/v1/artists/{id}/statistics`
- `POST /api/v1/artists`
- `PUT /api/v1/artists/{id}`
- `DELETE /api/v1/artists/{id}`
- `GET /api/v1/albums`
- `GET /api/v1/artists/{artist_id}/albums`
- `GET /api/v1/albums/{id}`
- `POST /api/v1/albums/{id}/search`
- `POST /api/v1/albums`
- `PUT /api/v1/albums/{id}`
- `DELETE /api/v1/albums/{id}`
- `GET /api/v1/tracks`
- `GET /api/v1/albums/{album_id}/tracks`
- `GET /api/v1/artists/{artist_id}/tracks`
- `GET /api/v1/tracks/{id}`
- `POST /api/v1/tracks`
- `PUT /api/v1/tracks/{id}`
- `DELETE /api/v1/tracks/{id}`
- `GET /api/v1/system/status`
- `GET /api/v1/system/version`
- `GET /api/v1/system/tasks`
- `GET /api/v1/system/logs`
- `GET /api/v1/activity/queue`
- `GET /api/v1/activity/history`
- `GET /api/v1/activity/processing`
- `GET /api/v1/events` (SSE)
- `GET /api/v1/events/connections`
- `POST /api/v1/events/broadcast`
- `GET /api/v1/events/download-progress` (SSE)
- `GET /api/v1/events/import-progress` (SSE)
- `GET /api/v1/events/job-status` (SSE)
- `GET /api/v1/settings/quality-profiles`
- `GET /api/v1/settings/quality-profiles/{id}`
- `POST /api/v1/settings/quality-profiles`
- `PUT /api/v1/settings/quality-profiles/{id}`
- `DELETE /api/v1/settings/quality-profiles/{id}`
- `GET /api/v1/settings/metadata-profiles`
- `GET /api/v1/settings/metadata-profiles/{id}`
- `POST /api/v1/settings/metadata-profiles`
- `PUT /api/v1/settings/metadata-profiles/{id}`
- `DELETE /api/v1/settings/metadata-profiles/{id}`
- `GET /api/v1/settings/download-clients`
- `GET /api/v1/settings/download-clients/{id}`
- `POST /api/v1/settings/download-clients`
- `PUT /api/v1/settings/download-clients/{id}`
- `DELETE /api/v1/settings/download-clients/{id}`
- `GET /api/v1/settings/indexers`
- `GET /api/v1/settings/indexers/{id}`
- `POST /api/v1/settings/indexers`
- `PUT /api/v1/settings/indexers/{id}`
- `DELETE /api/v1/settings/indexers/{id}`
- `POST /api/v1/indexers/test`

## Testing with the Mock Server

Integration tests for the `chorrosion-metadata` crate require a mock server running on `127.0.0.1:3030` **before** running tests. The test suite does not start or stop the server automatically.

- Start the mock server: `cargo run --bin mock_server &`
- Wait for it to be ready (see `crates/chorrosion-metadata/tests/README.md` for a helper).
- Run tests: `cargo test -p chorrosion-metadata`

See `crates/chorrosion-metadata/tests/README.md` for details and a helper function to wait for server readiness.

## Performance Benchmarks

End-to-end style API benchmarks are available in `chorrosion-api` (Criterion-based):

```bash
cargo bench -p chorrosion-api
```

For CI or quick compile checks without executing benchmark loops:

```bash
cargo bench -p chorrosion-api --no-run
```

## License

- License: GPL-3.0-or-later. See [LICENSE](LICENSE).

Adding endpoints generally involves:

1. Implementing a handler under `crates/chorrosion-api/src/handlers`.
2. Annotating with `#[utoipa::path]` and defining request/response types.
3. Wiring the route in `router()`.
4. Listing the path/schema in the `ApiDoc` derives.

## Database & Migrations

- SQLx with `SqlitePool` by default; Postgres support planned.
- Migrations are embedded via `sqlx::migrate!` and applied at startup from `./migrations`.
- Domain IDs (e.g., `ArtistId`) wrap `Uuid` and serialize as strings; stored as `TEXT` in SQLite.

## Scheduler & Jobs

- Jobs implement a `Job` trait with `job_type()`, `name()`, and `execute()` plus retry behavior.
- Registered via a `JobRegistry`; schedules are typically interval-based.
- Concurrency is limited by `SchedulerConfig.max_concurrent_jobs` using a semaphore.

To add a job:

1. Implement the job in `crates/chorrosion-scheduler/src/jobs.rs` (or a new module).
2. Register it in `Scheduler::register_jobs()` with an appropriate `Schedule`.

## Project Structure

```txt
crates/
  chorrosion-cli/           # Entrypoint: tracing, config load, migrations, scheduler, Axum server
  chorrosion-api/           # Routes, versioning, OpenAPI, handlers, middleware
  chorrosion-application/   # Application state (e.g., AppState with AppConfig)
  chorrosion-config/        # AppConfig + Figment wiring
  chorrosion-domain/        # IDs, enums, entities
  chorrosion-infrastructure/# Repository traits + SQLx adapters (stubs currently)
  chorrosion-realtime/      # Realtime placeholder
  chorrosion-scheduler/     # Job trait, job registry, schedules
migrations/                 # SQL migrations applied at startup
```

## Development

Common tasks:

```bash
# Build
cargo build

# Lint (Deny warnings)
cargo clippy -- -D warnings

# Format
cargo fmt

# Test (unit tests typically live next to code)
cargo test

# Rust 2024 readiness (fail on compatibility warnings)
# Bash/Zsh
RUSTFLAGS='-Wrust-2024-compatibility -Dwarnings' cargo check --workspace --all-targets

# PowerShell
$env:RUSTFLAGS="-Wrust-2024-compatibility -Dwarnings"; cargo check --workspace --all-targets
```

Logging:

- Set `RUST_LOG`, e.g., `info,api=debug,registry=debug`.
- Targets in use: `cli`, `api`, `application`, `infrastructure`, `scheduler`, `registry`, `jobs`, `auth`, `config`, `repository`.

## Cross-Platform Notes

- All file system ops use `std::path::Path` and `PathBuf`; avoid hardcoded separators.
- Paths are displayed via `path.display()`.
- Signal handling:
  - Unix: SIGINT and SIGTERM via `tokio::signal::unix`.
  - Windows: `tokio::signal::ctrl_c()`.
- Use `.gitattributes` with `* text=auto` to normalize line endings.

## Roadmap & Design

- High-level roadmap: see [ROADMAP.md](ROADMAP.md)
- Design notes and conventions: see [DESIGN.md](DESIGN.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines. Issues and PRs are welcome. Keep changes minimal and aligned with the workspace structure and tracing targets. Ensure code builds and runs on Windows, Linux, and macOS.

---
If you’re adding endpoints, jobs, or repositories, follow the patterns in the corresponding crates and keep API/OpenAPI in sync.
