# Contributing to Chorrosion

Thanks for your interest in improving Chorrosion! This guide helps you get productive quickly and keep changes consistent.

## Getting Started

- Install the Rust stable toolchain and `cargo`.
- Run the app locally:
  - PowerShell

    ```powershell
    $env:RUST_LOG="info,api=debug,registry=debug"
    $env:CHORROSION_DATABASE__URL="sqlite://data/chorrosion.db"
    cargo run -p chorrosion-cli
    ```

  - Bash/Zsh

    ```bash
    RUST_LOG=info,api=debug,registry=debug \
    CHORROSION_DATABASE__URL=sqlite://data/chorrosion.db \
    cargo run -p chorrosion-cli
    ```

## Development Standards

- Cross-platform: code must build and run on Windows, Linux, and macOS.
- Linting: `cargo clippy -- -D warnings` must pass.
- Formatting: run `cargo fmt` before committing.
- Tests: add unit tests near code when reasonable; run `cargo test`.
- Tracing: use existing targets (e.g., `cli`, `api`, `scheduler`) consistently.
- API/OpenAPI: keep routes, handlers, and OpenAPI in sync.
- Minimal changes: prefer focused, incremental PRs.

## Project Architecture (quick reference)

- CLI: sets tracing, loads config, runs migrations, starts scheduler + Axum.
- API: Axum routes, middleware, OpenAPI (`/docs`), health (`/health`).
- Config: Figment; env prefix `CHORROSION_` with `__` nesting.
- Data: SQLx (`SqlitePool`), migrations in `./migrations` applied at startup.
- Scheduler: jobs with interval schedules, retries, and concurrency limits.

## Branching & PRs

- Branch from `main`.
- Write a clear PR description: what, why, and how tested.
- Ensure CI passes on Ubuntu, Windows, and macOS.
- Reference issues when applicable.

## Common Commands

```bash
cargo build
cargo clippy -- -D warnings
cargo fmt
cargo test
```

## Reporting Issues

- Include OS, Rust version, steps to reproduce, and logs (set `RUST_LOG` as needed).

Thanks again for contributing!
