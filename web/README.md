# Chorrosion Web UI

SvelteKit frontend for Chorrosion, built with [Bun](https://bun.sh) and [SvelteKit](https://kit.svelte.dev).

## Prerequisites

- [Bun](https://bun.sh) (package manager and runtime)
- Chorrosion backend running at `http://127.0.0.1:5150` (or configure via env)

## Setup

Install dependencies:

```sh
bun install
```

Copy the env example and edit as needed:

```sh
cp .env.example .env
```

The only variable required for local development:

```text
VITE_CHORROSION_API_BASE=http://127.0.0.1:5150
```

## Developing

Start the Vite dev server (proxies API calls to the running Rust backend):

```sh
bun run dev
```

The dev server starts at `http://localhost:5173`. Make sure the Rust backend is also running:

```sh
# from the repo root
CHORROSION_AUTH__FORMS_COOKIE_SECURE=false cargo run -p chorrosion-cli
```

## Building

Create the production static build (output goes to `build/`):

```sh
bun run build
```

The output can be served directly by the Rust backend — see the root README for
`CHORROSION_WEB__SERVE_STATIC_ASSETS` configuration.

## Type-checking

```sh
bun run check
```

## Production with Rust backend

Build the frontend and configure the backend to serve it:

```sh
bun run build
# then from the repo root:
CHORROSION_WEB__SERVE_STATIC_ASSETS=true CHORROSION_WEB__STATIC_DIST_DIR=web/build cargo run -p chorrosion-cli
```
