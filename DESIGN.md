# Lidarr-Rust • Design Notes

Scope: design guidance for recently added roadmap items — fingerprint-first matching (MusicBrainz/AcoustID), embedded tag behavior, and fanart.tv artwork integration — aligned with the current architecture (Axum, Tokio, SQLx, Figment, Scheduler).

---

## Matching Strategy (MusicBrainz)
- Roadmap: [2.1 MusicBrainz Integration](ROADMAP.md#21-musicbrainz-integration)
- Primary: Chromaprint/AcoustID → MusicBrainz IDs (MBIDs)
  - Compute fingerprints during import/scan via a worker job.
  - Resolve AcoustID → MB recording IDs, then derive release/release-group, artist.
  - Use confidence thresholds; handle multi-match tie-breakers with duration/trackno/title similarity.
- Fallbacks (precedence): embedded tags → filename heuristics.
- Persistence:
  - Store MBIDs on entities (track/recording, release/album, artist).
  - Cache AcoustID responses with expiry to reduce rate usage.
- Resilience:
  - Backoff + retry on HTTP timeouts; respect remote rate limits.
  - Queue fingerprints for retry when confidence < configured threshold.

Implementation notes
- Job flow: `Scanner` enqueues "fingerprint" tasks; `MetadataRefresh` resolves to MB and updates entities.
- API additions: endpoints for rescanning an item’s fingerprint and viewing match confidence.
- Libraries: evaluate Rust Chromaprint bindings for fingerprint generation; simple HTTP client with `reqwest` for AcoustID/MB.

Configuration
- `AppConfig.metadata.matching.primary = "fingerprint"` (fixed) with thresholds and retry caps.
- `AppConfig.integrations.acoustid.api_key` (env: `LIDARR_INTEGRATIONS__ACOUSTID__API_KEY`).
- Optional: `max_concurrent_fingerprints`, `min_confidence`.

Related
- Fingerprint-first matching epic — issue: TBD
- AcoustID integration — issue: TBD
- Confidence/tie-breaker rules — issue: TBD
- Rescan and match visibility endpoints — issue: TBD

---

## Embedded Tags Behavior
- Roadmap: [5.4 Tagging & Embedding](ROADMAP.md#54-tagging--embedding)
- Modes
  - Preserve: read embedded tags/art for display/matching; never modify files.
  - Overwrite: after successful normalization (MB match), write canonical tags and embed selected artwork.
  - Read-only safety: global switch to prevent any file writes.
- Matching usage
  - If fingerprint match fails or is below threshold, use embedded tags for MB search.
  - Use embedded art as a UI/display fallback when network art is unavailable.
- Write discipline
  - Atomic writes: write to temp + fsync + rename; keep optional backup.
  - Tag sanitation: normalize unicode, remove illegal/duplicate frames, clamp text length.
  - Artwork sizing: downscale/convert per config (JPEG/PNG, max dimension/bytes).

Implementation notes
- Candidate libraries: consider `lofty` for multi-format tag read/write; `id3` crate for MP3-specific robustness; evaluate format coverage and write guarantees across Windows/Linux/macOS.
- Unsupported formats fall back to external sidecar or skip (configurable).

Configuration
- `AppConfig.tags.mode = "preserve" | "overwrite"`.
- `AppConfig.tags.read_only = true|false`.
- `AppConfig.tags.artwork.max_dimension`, `max_bytes`, `format_preference`.

Related
- Tag overwrite preference (preserve vs overwrite) — issue: TBD
- Read-only safety mode — issue: TBD
- Cross-format write support matrix — issue: TBD

---

## Artwork: fanart.tv Integration
- Roadmap: [2.2 Additional Metadata Sources](ROADMAP.md#22-additional-metadata-sources)
- Purpose: alternate/high-quality artwork source to complement Cover Art Archive.
- Data types: artist backgrounds, logos, album covers; choose best locale/resolution per config.
- API
  - Requires API key; cache responses and image assets.
  - Respect fanart.tv rate limits and attribution requirements.
- Selection order (example)
  - Cover Art Archive → fanart.tv → embedded art → placeholder.
- Storage & caching
  - Cache directory with hashed paths; validate content-type + size; periodic cleanup job.

Implementation notes
- HTTP via `reqwest` with timeouts; exponential backoff.
- Map fanart.tv JSON to internal `Artwork` model (type, url, lang, size, rating).

Configuration
- `AppConfig.integrations.fanart.api_key` (env: `LIDARR_INTEGRATIONS__FANART__API_KEY`).
- `AppConfig.artwork.source_order = ["coverartarchive", "fanart", "embedded"]`.
- `AppConfig.artwork.pref_language`, `min_width`.

Related
- fanart.tv integration (client + models) — issue: TBD
- Artwork caching and cleanup job — issue: TBD
- Artwork selection order and sizing policy — issue: TBD

---

## Scheduler & Jobs
- New/updated jobs
  - `FingerprintJob`: compute + persist fingerprints, enqueue MB resolution.
  - `MetadataRefreshJob`: resolve AcoustID→MB, update entities, schedule art fetch.
  - `ArtworkFetchJob`: fetch from configured sources, store and validate art.
- Concurrency & retries
  - Controlled by `SchedulerConfig.max_concurrent_jobs`; per-job retry/backoff.

---

## API Surface (Additions)
- GET `/api/v1/tracks/{id}/match` → current match details (confidence, sources used).
- POST `/api/v1/tracks/{id}/fingerprint` → trigger fingerprint (idempotent enqueue).
- POST `/api/v1/tracks/{id}/retag` → apply overwrite mode to a single file.
- GET `/api/v1/system/settings/tags` → view tag/embedding settings.

Document in OpenAPI via `utoipa`, ensure components reflect new models.

---

## Testing Strategy
- Unit tests: mapping, precedence logic, confidence thresholds, art selection.
- Integration tests: mock AcoustID/MB/fanart.tv with recorded fixtures; temp directories for tag write/read round-trips; cover Windows path semantics.
- Property tests for filename heuristics and tag sanitation.

---

## Risks & Mitigations
- Platform-specific file locking (Windows): use temp+rename pattern and close handles promptly.
- Tag write compatibility: gate writes behind format support checks; offer read-only mode by default initially.
- Third-party rate limits: caching, jittered backoff, and budgeted concurrency.

---

## Open Questions
- Exact library choices for cross-format tag writing (pilot with `lofty`, fall back to format-specific crates?).
- Default artwork precedence and size constraints.
- Where to persist downloaded art (DB vs filesystem with hashed paths + DB pointers).

---

References
- CLI entry and lifecycle: crates/lidarr-cli/src/main.rs
- API surface and OpenAPI: crates/lidarr-api/src/lib.rs
- App state/config: crates/lidarr-application/src/lib.rs, crates/lidarr-config/src/lib.rs
- Scheduler: crates/lidarr-scheduler/src/*
- Data layer: crates/lidarr-infrastructure/src/*