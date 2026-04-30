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

## Frontend Development (web/)

The frontend is a SvelteKit 2 app using Svelte 5 runes, TypeScript, and Bun. It lives in the `web/` directory.

### Setup

Install [Bun](https://bun.sh) (v1.x), then:

```bash
cd web
bun install
```

### Validation commands

Run these before opening a PR that touches `web/`:

```bash
# Type-check all Svelte components and TypeScript files
bun run check

# Run unit tests with Vitest
bun run test

# Production build (adapter-static → web/build/)
bun run build
```

All three must pass cleanly. The `frontend-ci.yml` workflow enforces them on Ubuntu and Windows for every pull request that modifies files under `web/`.

### Adding tests

Tests live next to the source they cover, named `*.test.ts`:

- `src/lib/auth.test.ts` — unit tests for auth store utilities
- `src/lib/api.test.ts` — unit tests for `ApiError` and `sseUrl`

Use [Vitest](https://vitest.dev) globals (`describe`, `it`, `expect`) and [@testing-library/svelte](https://testing-library.com/docs/svelte-testing-library/intro) for component tests. The DOM environment is `happy-dom`.

### Frontend conventions

- Svelte 5 runes (`$state`, `$derived`, `$effect`); no Options API or legacy stores in new components.
- API calls go through `src/lib/api.ts`; auth state through `src/lib/auth.ts`.
- CSS custom properties from `app.css` for theming — no hardcoded colours.
- Cross-platform: all paths and environment variables work on Windows and Linux.

## Reporting Issues

- Include OS, Rust version, steps to reproduce, and logs (set `RUST_LOG` as needed).

Thanks again for contributing!
