# Lidarr-Rust • Copilot Instructions (Agent Guide)

Purpose: Give AI agents the minimum, code-proven context to be productive fast. Prefer concrete patterns from this repo over generic advice.

## Project Snapshot
- Backend: Axum + Tokio; workspace with focused crates.
- DB: SQLx SqlitePool (default SQLite, Postgres support planned).
- Config: Figment (defaults → optional TOML → env `LIDARR_` with `__` nesting).
- API: utoipa OpenAPI + Swagger UI at `/docs`; health at `/health`.
- Jobs: Tokio-based scheduler with concurrency and retry controls.
- Platform: Cross-platform (Windows, Linux, macOS) with consistent behavior.

## Run & Debug
- Dev server: `cargo run -p lidarr-cli`
  - Default bind: `127.0.0.1:5150` (see `HttpConfig`).
  - Example: `RUST_LOG=info LIDARR_DATABASE__URL=sqlite://data/lidarr.db cargo run -p lidarr-cli`
- Logs: Env filter via `RUST_LOG` (e.g., `RUST_LOG=info,api=debug,registry=debug`).
- Migrations: Applied at startup from `./migrations` via `sqlx::migrate!` (no compile-time DB needed).

## Architecture Map (where things live)
- CLI entry: crates/lidarr-cli/src/main.rs → sets tracing, loads config, runs migrations, starts scheduler, serves Axum.
- API surface: crates/lidarr-api/src/lib.rs → routes, versioning, OpenAPI; handlers in handlers/*, auth in middleware/*.
- Application state: crates/lidarr-application/src/lib.rs → `AppState` (currently holds `AppConfig`).
- Domain model: crates/lidarr-domain/src/lib.rs → IDs (`ArtistId`, etc.), enums, entities; IDs wrap `Uuid` and serialize as strings.
- Data layer: crates/lidarr-infrastructure/src/* → repository traits + SQLx adapters (currently stubs with tracing).
- Scheduler: crates/lidarr-scheduler/src/* → `Job` trait, `JobRegistry`, canned jobs, interval-based schedules.
- Config: crates/lidarr-config/src/lib.rs → `AppConfig` + env/TOML loading.

## Conventions & Patterns
- API
  - Versioned under `/api/v1`; auth middleware applied to API router.
  - Handlers return typed JSON and are annotated for OpenAPI (see artists endpoints in handlers/artists.rs).
  - Add endpoints by: defining handler + request/response types, annotating with `#[utoipa::path]`, wiring route in `router()`, and listing path/schema in `ApiDoc` derives.
- Jobs
  - Implement `Job` with `job_type()`, `name()`, `execute()`, retry behavior; register in `Scheduler::register_jobs()` with a `Schedule::Interval(seconds)`.
  - Concurrency is limited by `SchedulerConfig.max_concurrent_jobs` via a semaphore in `JobRegistry`.
- IDs & DB
  - Domain IDs are strongly typed newtypes over `Uuid` but stored as TEXT in SQLite (see migrations/20260106000000_initial_schema.sql). API DTOs use string IDs.
- Tracing
  - Targets in use: `cli`, `api`, `application`, `infrastructure`, `scheduler`, `registry`, `jobs`, `auth`, `config`, `repository`.

## Configuration
- Source order: code defaults → optional TOML file (path wiring pending in CLI) → env `LIDARR_` with `__` nesting.
  - Examples: `LIDARR_DATABASE__URL`, `LIDARR_HTTP__HOST`, `LIDARR_HTTP__PORT`, `LIDARR_SCHEDULER__MAX_CONCURRENT_JOBS`.
- SQLite convenience: parent dir auto-created when URL starts with `sqlite://`.

## Common Tasks
- Build: `cargo build`
- Lint: `cargo clippy -- -D warnings`
- Format: `cargo fmt`
- Test: `cargo test` (tests not yet populated; add unit tests next to code).
- SQLx offline: not required currently (no `query!` macros); migrations are embedded via `migrate!`.

## How to Extend (examples)
- New endpoint: add handler in `crates/lidarr-api/src/handlers`, annotate with utoipa, wire in `router()`, add to `ApiDoc` `#[openapi(paths(...), components(...))]` lists.
- New job: implement `Job` in `crates/lidarr-scheduler/src/jobs.rs` (or new module), then register in `Scheduler::register_jobs()` with an interval.
- Repository impl: implement trait(s) from `crates/lidarr-infrastructure/src/repositories.rs` in a new adapter (use `SqlitePool`) and inject where used.

## Integration Points (planned but stubbed)
- Indexers (Torznab/Newznab/Gazelle), download clients, and MusicBrainz integration are not wired yet; use trait-first designs in infrastructure and application layers when adding.

## Cross-Platform Compatibility (Windows, Linux, macOS)
**Critical:** All code must compile and run correctly on Windows, Linux, and macOS without platform-specific bugs.

### Database Path Handling
- **SQLite URLs:** Always normalize paths to absolute paths with forward slashes (SQLite handles this on all platforms).
- **Create mode:** Use `?mode=rwc` query parameter to allow SQLite to create database files.
- **Implementation pattern:**
  ```rust
  // Convert relative to absolute, replace backslashes with forward slashes
  let absolute_path = std::env::current_dir()?.join(relative_path);
  let path_str = absolute_path.to_string_lossy().replace('\\', "/");
  let db_url = format!("sqlite://{}?mode=rwc", path_str);
  ```
- **Directory creation:** Always call `std::fs::create_dir_all()` for parent directories before connecting to SQLite.
- See `crates/lidarr-infrastructure/src/lib.rs::init_database()` for reference implementation.

### Signal Handling
- Use `#[cfg(unix)]` and `#[cfg(not(unix))]` attributes for platform-specific signal handling.
- Unix: Handle both SIGINT and SIGTERM via `tokio::signal::unix`.
- Windows: Use `tokio::signal::ctrl_c()` only.
- See `crates/lidarr-cli/src/main.rs::shutdown_signal()` for reference implementation.

### Path Separators
- **General rule:** Use `std::path::Path` and `PathBuf` for all file system operations.
- **Never hardcode:** Avoid hardcoded `/` or `\` in paths; use `Path::join()` instead.
- **Display:** When logging paths, use `path.display()` for correct platform representation.

### Testing Cross-Platform
- **Local testing:** Test on your development platform regularly.
- **CI/CD:** Ensure GitHub Actions or equivalent CI runs on both `ubuntu-latest` and `windows-latest`.
- **Common issues to watch for:**
  - Path separators in string literals
  - Case-sensitive file systems (Linux/macOS vs Windows)
  - Line endings (LF vs CRLF) - use `.gitattributes` with `* text=auto`
  - Signal handling and process termination
  - File permissions and locking behavior

### Environment Variables
- Format: `LIDARR_SECTION__KEY` (double underscore for nesting)
- Works identically on all platforms via Figment
- PowerShell: `$env:LIDARR_DATABASE__URL="sqlite://data/lidarr.db"`
- Bash/Zsh: `LIDARR_DATABASE__URL=sqlite://data/lidarr.db`

---
Notes for agents
- Favor minimal changes, align with existing tracing targets and module layout.
- Keep API/OpenAPI in sync; add schemas to `components` and paths to `paths`.
- Respect scheduler limits; long jobs should yield (`sleep`) to avoid blocking.

Last Updated: 2026-01-07
