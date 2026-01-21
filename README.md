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

SQLite tips:

- Use `?mode=rwc` in URLs if you need create-on-open semantics.

## API

- Base: `/api/v1`
- OpenAPI docs are generated with `utoipa` and exposed at `/docs`.

## Testing with the Mock Server

Integration tests for the `chorrosion-metadata` crate require a mock server running on `127.0.0.1:3030` **before** running tests. The test suite does not start or stop the server automatically.

- Start the mock server: `cargo run --bin mock_server &`
- Wait for it to be ready (see `crates/chorrosion-metadata/tests/README.md` for a helper).
- Run tests: `cargo test -p chorrosion-metadata`

See `crates/chorrosion-metadata/tests/README.md` for details and a helper function to wait for server readiness.

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
