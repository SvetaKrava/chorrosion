# Chorrosion Development Roadmap

## Current Status (v0.1.0) ✅

- [x] Project structure and workspace setup
- [x] Database schema and migrations (SQLite)
- [x] Configuration management (Figment with env/TOML support)
- [x] Cross-platform compatibility (Windows, Linux, macOS)
- [x] API framework with OpenAPI/Swagger documentation
- [x] Background job scheduler with retry logic
- [x] Basic health endpoints
- [x] Artist CRUD endpoint stubs
- [x] Repository pattern with SQLite adapters
- [x] Graceful shutdown handling
- [x] Structured logging with tracing

---

## Phase 1: Core Data Layer (Q1 2026)

### 1.1 Repository Implementation ✅ COMPLETE

- [x] Implement Artist repository with full CRUD
  - [x] Create/Read/Update/Delete operations
  - [x] Filtering by status, monitored state
  - [x] Search by name and foreign ID
- [x] Implement Album repository
  - [x] CRUD operations with artist relationships
  - [x] Filtering by status, release dates
  - [x] Album type handling (studio, live, compilation)
- [x] Implement Track repository
  - [x] CRUD with album/artist relationships
  - [x] Track file associations
  - [x] Duration and track numbers
- [x] Implement Quality Profile repository
- [x] Implement Metadata Profile repository

All core data layer repositories complete with comprehensive CRUD operations, specialized queries, and full test coverage.

### 1.2 Domain Models Enhancement ✅ COMPLETE

- [x] Add validation logic to entities
- [x] Implement domain events for state changes
- [x] Add file path generation logic
- [x] Implement track file model with quality info
- [x] Add release date handling and parsing ✓ (Issue #23)

All Phase 1.2 domain enhancements complete: validation traits, event bus with Artist/Album/Track/TrackFile events, ReleaseDate parsing, path generation utilities, and TrackFile model.

---

## Phase 2: Metadata Integration (Q1-Q2 2026)

### 2.1 MusicBrainz Integration

_(See design: [Matching Strategy](DESIGN.md#matching-strategy-musicbrainz))_

- [x] MusicBrainz API client implementation
  - [x] Artist search and lookup
  - [x] Album (release group) search and lookup
  - [x] Recording (track) lookup ✓ (Issue #26)
  - [x] Cover art fetching ✓ (Issue #26)
  - [x] Fingerprint-based matching (Chromaprint/AcoustID) as primary
    - [x] Generate audio fingerprints during import/scan
      - [x] Phase 1: Core framework and format detection (Issue #65, PR #90) ✓
        - [x] FingerprintGenerator module with async API
        - [x] FLAC/MP3 format detection and routing
        - [x] AudioSamples container with 120s duration limiting
        - [x] Comprehensive error handling and test coverage
      - [x] Phase 2: Symphonia-based audio decoding (Issue #65) ✓
        - [x] Implement FLAC audio sample extraction ✓
        - [x] Implement MP3 audio sample extraction ✓
        - [x] Chromaprint fingerprint generation ✓
      - [x] Advanced formats: OGG, Opus, WavPack, APE (Issue #89, PR #98 - FFmpeg optional)
    - [x] Resolve via AcoustID to MusicBrainz IDs (MBIDs) ✓ (Issue #25)
    - [x] Confidence thresholds and tie-breakers ✓
    - [x] Database schema for fingerprint storage (Issue #66) ✓
    - [x] File import integration with fingerprint generation (Issue #67) ✓
    - [x] Primary matching engine: fingerprint lookup + MBID linking (Issue #68) ✓
      - [x] Advanced format support via FFmpeg (Issue #89, PR #98) ✓ COMPLETE
      - [x] Optional ffmpeg-support feature flag ✓
      - [x] OGG Vorbis, Opus, WavPack, APE, DSF, M4A, AAC support ✓
      - [x] Graceful fallback when FFmpeg unavailable ✓
    - [ ] Embed fingerprint in audio file tags (part of Phase 5.4)
    - [x] Embedded tags extraction interface ✓
    - [x] Filename heuristics parsing with regex patterns ✓
    - [x] Matching fallback chain documentation ✓
- [x] Metadata refresh jobs ✓ (Issue #27)
  - [x] Scheduled artist metadata updates ✓
  - [x] Album metadata updates ✓
  - [x] Rate limiting and caching ✓
- [x] Metadata mapping (Issue #2, PR #92) ✓ PARTIALLY COMPLETE
  - [x] MusicBrainz ID storage (Artist/Album) ✓
  - [x] Rich metadata fields (type, sort_name, country, etc.) ✓
  - [x] Database schema with indices ✓
  - [x] Domain model updates ✓
  - [x] Repository layer persistence ✓
  - [x] Genre and style mapping (Issue #2, PR #93) ✓ COMPLETE
    - [x] Database migration with genre/style columns ✓
    - [x] Domain model updates ✓
    - [x] Repository layer updates ✓
    - [x] Integration tests for genre/style persistence ✓
    - [x] PR creation and merge ✓
  - [x] Artist relationships (Issue #68, PR #94) ✓ COMPLETE
    - [x] Database schema for artist_relationships table ✓
    - [x] ArtistRelationship domain model ✓
    - [x] ArtistRelationshipRepository trait with specialized queries ✓
    - [x] SqliteArtistRelationshipRepository implementation ✓
    - [x] Query methods: by source/related artist, by type, existence check ✓
    - [x] 4 comprehensive integration tests ✓
    - [x] PR creation and merge ✓
  - [x] Matching precedence enforcement: fingerprint > embedded tags > filename (PR #97) ✓ COMPLETE
    - [x] PrecedenceMatchingEngine orchestrator ✓
    - [x] Strategy precedence ordering (fingerprint > tags > filename) ✓
    - [x] Confidence thresholds and fallback logic ✓
    - [x] Comprehensive test coverage (27 tests) ✓
    - [x] Cross-platform compatibility verified ✓
    - [x] PR created and merged ✓

### 2.2 Additional Metadata Sources

- [x] Last.fm integration for additional metadata (Issue #101, PR #114) ✓ COMPLETE
  - [x] Implement Last.fm API client (Issue #106, PR #111) ✓
    - [x] Fetch artist metadata ✓
    - [x] Fetch album metadata ✓
    - [x] Rate limiting and caching ✓
    - [x] Integration tests ✓
  - [x] Scheduler-driven metadata refresh (PR #114) ✓
- [x] Discogs integration (optional) (Issue #102, PR #119) ✓ COMPLETE
  - [x] Implement Discogs API client (Issue #107, PR #117) ✓
    - [x] Fetch artist metadata ✓
    - [x] Fetch album metadata ✓
    - [x] Authentication setup ✓
    - [x] Integration tests ✓
  - [x] Scheduler-driven metadata refresh (PR #119) ✓
- [x] Fanart.tv artwork integration (alternative art source) (Issue #103, PR #122) ✓ COMPLETE
  - [x] Cover art fallback sources (Issue #127, PR #129) ✓ COMPLETE
  - [x] Lyrics fetching (optional enhancement) (Issue #104, PR #124) ✓ COMPLETE

---

## Phase 3: Indexer Integration (Q2 2026)

### 3.1 Indexer Framework

- [x] Indexer configuration model (Issue #29, PR #133) ✓ COMPLETE
- [x] Indexer trait definition (Issue #29, PR #133) ✓
- [x] Indexer capability detection (Issue #29, PR #133) ✓
- [x] Indexer testing endpoints (Issue #29, PR #133) ✓

### 3.2 Protocol Implementations

- [x] Newznab protocol client (Issue #30, PR #136) ✓ COMPLETE
  - [x] Search capabilities ✓
  - [x] RSS feed parsing ✓
  - [x] Category mapping ✓
- [x] Torznab protocol client (Issue #30, PR #136) ✓ COMPLETE
  - [x] Torrent-specific handling ✓
  - [x] Magnet link support ✓
- [ ] Gazelle protocol client (optional)
  - [ ] API authentication
  - [ ] Music-specific search

### 3.3 Release Parsing

- [x] Release title parser (Issue #31, PR #139) ✓ COMPLETE
  - [x] Artist/album extraction ✓
  - [x] Quality detection (MP3, FLAC, etc.) ✓
  - [x] Bitrate parsing ✓
  - [x] Release group detection ✓
- [x] Release filtering and ranking (Issue #31, PR #139) ✓ COMPLETE
- [x] Duplicate detection (Issue #141, PR #143) ✓ COMPLETE

---

## Phase 4: Search & Download (Q2-Q3 2026)

### 4.1 Search Functionality

- [x] Manual search implementation (Issue #33, PR #145) ✓ PARTIALLY COMPLETE
  - [x] Artist search ✓
  - [x] Album search ✓
  - [ ] Interactive search UI support
- [x] Automatic search (Issue #33, PR #145) ✓ COMPLETE
  - [x] Missing album detection ✓
  - [x] Search triggering logic ✓
  - [x] Best release selection algorithm ✓
- [ ] RSS sync enhancement
  - [ ] Parse RSS feeds from indexers
  - [ ] Match releases to wanted albums
  - [ ] Automatic grab logic

### 4.2 Download Client Integration

- [x] Download client trait definition (Issue #32, PR #148) ✓ COMPLETE
- [x] qBittorrent client (Issue #32, PR #148) ✓ COMPLETE
  - [x] Add torrent support ✓
  - [x] Category management ✓
  - [x] Status monitoring ✓
- [x] Transmission client (Issue #349) ✓
- [ ] Deluge client
- [ ] SABnzbd client (Usenet)
- [ ] NZBGet client (Usenet)
- [x] Download queue management (Issue #32, PR #148) ✓

### 4.3 Download Monitoring

- [ ] Download status tracking
- [ ] Completion detection
- [ ] Failed download handling
- [ ] Stalled download detection
- [ ] Download history

---

## Phase 5: File Management (Q3 2026)

### 5.1 Import System

- [x] File scanning and detection (Issue #34, PR #152, PR #153) ✓ COMPLETE
- [x] Track file parsing (tags, duration, bitrate) (Issue #34, PR #152, PR #153) ✓ PARTIALLY COMPLETE
  - [x] Embedded tag field model (artist, album, title) ✓
  - [x] Duration read from file system metadata ✓
  - [x] Bitrate extraction from filename heuristics (BITRATE_REGEX) ✓
  - [ ] Library-based tag reading (ID3/FLAC/Vorbis/APEv2) — placeholder only
  - [ ] Bitrate from audio codec/stream data (not just filename)
- [x] **Fingerprint generation during import** (Issue #67) ✓
  - [x] Generate Chromaprint fingerprint ✓
  - [x] Cache in database (Issue #66) ✓
  - [x] Store in TrackFile domain model ✓
- [x] File matching algorithm (Issue #34, PR #152, PR #153) ✓ PARTIALLY COMPLETE
  - [x] **Primary: Fingerprint-based lookup** (Issue #68) ✓
    - [x] Query AcoustID with fingerprint ✓
    - [x] Link to MusicBrainz recording ✓
    - [ ] Link to artist/album via recording
  - [x] Fallback: Embedded tag matching (Issue #28) ✓
  - [x] Fallback: Filename heuristics (Issue #28) ✓
  - [x] Fuzzy matching for poor metadata (Issue #34, PR #152, PR #153) ✓
- [x] Import decision logic (Issue #34, PR #152, PR #153) ✓ PARTIALLY COMPLETE
  - [x] ImportDecision enum (Import / NeedsReview / Skip) ✓
  - [x] Confidence-threshold-based auto-import ✓
  - [x] NeedsReview flagging for low-confidence matches ✓
  - [x] Catalog match evaluation (ImportEvaluation) ✓
  - [x] Quality upgrade / duplicate-file decisions (Issue #334) ✓
  - [ ] Manual import UI support

### 5.2 File Organization

- [x] File renaming implementation (Issue #35, PR #156, PR #158) ✓ PARTIALLY COMPLETE
  - [x] File Path Generation and Naming Logic (Issue #21, PR #156, PR #158) ✓
  - [x] Naming pattern engine (Issue #35, PR #156, PR #158) ✓
  - [x] Token replacement (artist, album, track, disc, ext) (Issue #35, PR #156, PR #158) ✓
  - [x] Safe file operations (Issue #35, PR #156, PR #158) ✓
- [x] Folder organization (Issue #35, PR #156, PR #158) ✓ COMPLETE
  - [x] Artist folder structure (Issue #35, PR #156, PR #158) ✓
  - [x] Album folder structure (Issue #35, PR #156, PR #158) ✓
  - [x] Multi-disc handling (Issue #35, PR #156, PR #158) ✓
- [x] File operations (Issue #35, PR #156, PR #158) ✓ PARTIALLY COMPLETE
  - [x] Copy vs. move logic (Issue #35, PR #156, PR #158) ✓
  - [x] Hard link support (Issue #35, PR #156, PR #158) ✓
  - [x] Permission handling (Issue #336, Issue #337) ✓

### 5.3 Quality Management

- [x] Quality upgrades detection (Issue #332) ✓
  - [x] Compare existing vs. new quality ✓
  - [x] Upgrade decision logic (`QualityUpgradeService`) ✓
  - [x] Cutoff management (`QualityComparer::meets_cutoff`) ✓
- [x] File replacement workflow (`FileReplacementService`) (Issue #332) ✓
- [x] Backup of replaced files (configurable via `FileReplacementConfig`) (Issue #332) ✓

### 5.4 Tagging & Embedding

_(See design: [Embedded Tags Behavior](DESIGN.md#embedded-tags-behavior))_

- [x] Embed metadata and artwork in supported formats (Issue #36, PR #160, PR #165, PR #166) ✓ PARTIALLY COMPLETE
  - [x] ID3v2 (MP3): tags + front cover artwork (Issue #36, PR #165, PR #166) ✓
  - [x] Vorbis Comments (FLAC/OGG): tags + embedded pictures + fingerprint (Issue #36, PR #165, PR #166) ✓
  - [x] MP4/M4A atoms: tags + cover art (`covr`) + fingerprint (Issue #36, PR #165, PR #166) ✓
  - [x] Safe, atomic writes with backup/rollback on failure (Issue #36, PR #160, PR #161) ✓
  - [x] Charset/normalization handling and tag sanitation (Issue #339) ✓
  - [x] Configurable per profile (enable/disable, overwrite rules) (Issue #36, PR #160) ✓
  - [x] User preference: preserve existing embedded metadata/art (do not overwrite existing content) vs overwrite on import/refresh (Issue #342) ✓
  - [x] Read-only tag mode that never modifies source files (Issue #36, PR #160) ✓
  - [x] Fallback behavior for unsupported file types (Issue #36, PR #160) ✓
  - [x] **Store computed fingerprint in file tags** (Issue #36, PR #165, PR #166) ✓

---

## Phase 6: User Interface Enhancement (Q3-Q4 2026)

- Related issues: #9, #10, #11

### 6.1 API Completion

- [x] Artist endpoints (Issue #9, PR #169, PR #172) ✓
  - [x] List with pagination and sorting ✓
  - [x] Detailed artist view ✓
  - [x] Update monitored status (via update endpoint) ✓
  - [x] Artist statistics (`GET /api/v1/artists/{id}/statistics`) ✓
- [x] Album endpoints (PR #175) ✓ PARTIALLY COMPLETE
  - [x] Album list and details ✓
  - [x] Album create/update/delete ✓
  - [x] Monitor toggle (via update endpoint) ✓
  - [x] List by artist (`GET /api/v1/artists/{artist_id}/albums`) ✓
  - [x] Search trigger (`POST /api/v1/albums/{id}/search`) ✓
- [x] Track endpoints (PR #178) ✓ PARTIALLY COMPLETE
  - [x] Track list and details ✓
  - [x] Track create/update/delete ✓
  - [x] Album/artist filtered listing endpoints (`GET /api/v1/albums/{album_id}/tracks`, `GET /api/v1/artists/{artist_id}/tracks`) ✓
- [x] Queue/Activity endpoints ✓ PARTIALLY COMPLETE
  - [x] Download queue endpoint (`GET /api/v1/activity/queue`) ✓
  - [x] History endpoint (`GET /api/v1/activity/history`) ✓
  - [x] Currently processing endpoint (`GET /api/v1/activity/processing`) ✓
- [x] System endpoints ✓
  - [x] Status and version ✓
  - [x] Tasks/jobs management (`/api/v1/system/tasks`) ✓
  - [x] Log viewing (`/api/v1/system/logs`) ✓
- [x] Settings endpoints ✓ PARTIALLY COMPLETE
  - [x] Quality profiles CRUD (`/api/v1/settings/quality-profiles`) ✓
  - [x] Metadata profiles CRUD (`/api/v1/settings/metadata-profiles`) ✓
  - [x] Indexer management (`/api/v1/settings/indexers`) ✓
  - [x] Download client management (`/api/v1/settings/download-clients`) ✓

### 6.2 WebSocket/SSE Support

- [x] SSE event stream baseline (`GET /api/v1/events`) ✓
- [x] Real-time updates implementation ✓
  - [x] Download progress (`GET /api/v1/events/download-progress`) ✓
  - [x] Import progress (`GET /api/v1/events/import-progress`) ✓
  - [x] Job status (`GET /api/v1/events/job-status`) ✓
- [x] Event broadcasting (`POST /api/v1/events/broadcast`) ✓
- [x] Client connection management (`GET /api/v1/events/connections`) ✓

### 6.3 Authentication & Authorization

- [x] API key generation and management (`/api/v1/auth/api-keys`) ✓
- [x] Basic authentication support (`Authorization: Basic ...`) ✓
- [x] Forms authentication (optional) (Issue #344) ✓
- [x] Permission levels (optional) (Issue #346) ✓

---

## Phase 7: Advanced Features (Q4 2026)

### 7.1 Wanted/Missing Management

- [x] Wanted album tracking (Issue #223) ✓
- [x] Missing album detection (Issue #223) ✓
- [x] Cutoff unmet detection (Issue #226) ✓
- [x] Automated search scheduling (Issue #229) ✓
- [x] Manual search interface (Issue #232) ✓

### 7.2 Calendar

- [x] Upcoming releases calendar (Issue #235) ✓
- [x] Release date tracking (Issue #235) ✓
  - [x] Calendar API endpoints (Issue #235) ✓
  - [x] iCal feed support (Issue #235) ✓

### 7.3 Notifications

- [x] Notification framework (Issue #238) ✓
- [x] Email notifications (Issue #241) ✓
- [x] Discord webhook (Issue #244) ✓
- [x] Slack webhook (Issue #247) ✓
- [x] Pushover integration (Issue #250) ✓
- [x] Custom scripts support (Issue #253) ✓

### 7.4 Lists Integration

- [x] List provider trait (Issue #256) ✓
- [x] MusicBrainz list import (Issue #259) ✓
- [x] Spotify playlist import (optional) (Issue #261) ✓
- [x] Last.fm integration (Issue #264) ✓
- [x] Auto-add from lists (Issue #267) ✓

---

## Phase 8: Performance & Reliability (Ongoing)

- Related issues: #13, #37, #38, #39, #40, #41, #42

### 8.1 Performance Optimization

- [ ] Database query optimization
  - [x] Add indexes for common queries (Issue #270) ✓
  - [x] Query profiling and tuning (Issue #273) ✓
- [x] Caching layer (Issue #278) ✓
  - [x] Metadata caching ✓
  - [x] API response caching ✓
  - [x] File system cache ✓
- [x] Concurrent operation improvements (Issue #281) ✓
- [x] Memory usage optimization (Issue #285) ✓

### 8.2 Reliability

- [ ] Comprehensive error handling
  - [x] Retry logic for external APIs (Issue #288) ✓
  - [x] Timeout handling (Issue #291) ✓
  - [x] Rate limit handling (Issue #294) ✓
- [ ] Data integrity
  - [x] Database constraints (Issue #297) ✓
  - [x] Transaction management (Issue #300) ✓
  - [x] Backup/restore functionality (Issue #302) ✓
- [ ] Monitoring and observability
  - [x] Metrics collection (Prometheus) (Issue #306) ✓
  - [x] Health checks (Issue #308) ✓
  - [x] Performance tracing (Issue #310) ✓

### 8.3 Testing

- [ ] Unit test coverage
  - [x] Repository tests (Issue #316) ✓
  - [x] Business logic tests (Issue #314) ✓
  - [x] API endpoint tests (Issue #312) ✓
- [ ] Integration tests
  - [x] Database integration tests (Issue #318) ✓
  - [x] External API mock tests (Issue #320) ✓
- [ ] End-to-end tests
  - [x] Full workflow tests (Issue #322) ✓
  - [x] Performance benchmarks (Issue #324) ✓

### 8.4 Maintenance & Dependencies

- [ ] Address future-incompat warnings in dependencies
  - [x] Upgrade `sqlx`/`sqlx-postgres` to 0.8.x or newer to resolve never-type fallback warnings
  - [x] Gate PostgreSQL behind a feature and disable by default to avoid pulling incompatible crates until upgraded (Issue #326) ✓
  - [x] Add a CI job to run `cargo report future-incompatibilities --id 2` and fail on new findings (Issue #328) ✓
  - [x] Track Rust 2024 edition changes (e.g., never type fallback) and ensure readiness before edition bump (Issue #330) ✓

---

## Phase 9: PostgreSQL Support (TBD)

- Related issues: #12

### 9.1 Database Abstraction

- [ ] Abstract database-specific queries
- [ ] Add PostgreSQL-specific optimizations
- [ ] Migration compatibility
- [ ] Connection pooling tuning

### 9.2 Migration Tools

- [ ] SQLite to PostgreSQL migration tool
- [ ] Schema comparison tools
- [ ] Data validation after migration

---

## Phase 10: Optional Enhancements (Future)

### 10.1 Advanced Features

- [ ] Custom formats support
- [ ] Preferred word handling
- [ ] Release restrictions
- [ ] Tag-based organization
- [ ] Smart playlists
- [ ] Duplicate detection and management

### 10.2 Community Features

- [ ] Plugin system architecture
- [ ] Extension API
- [ ] Community indexer definitions
- [ ] Custom script hooks

### 10.3 UI Improvements

- [ ] Dark/light theme support
- [ ] Mobile-responsive design
- [ ] Keyboard shortcuts
- [ ] Bulk operations UI
- [ ] Advanced filtering

---

## Success Metrics

### Feature Parity Goals

- [ ] Parity with core Lidarr features where applicable
- [ ] Compatible with existing Lidarr API clients where feasible
- [ ] Support for major download clients
- [ ] Support for major indexers
- [ ] Complete metadata workflow

### Performance Goals

- [ ] Startup time < 2 seconds
- [ ] API response time < 100ms (p95)
- [ ] Support 10,000+ artists without degradation
- [ ] Memory usage < 200MB base + reasonable growth
- [ ] CPU usage < 5% idle

### Quality Goals

- [ ] 80%+ test coverage
- [ ] Zero known critical bugs
- [ ] All security vulnerabilities addressed
- [ ] Documentation complete
- [x] CI/CD pipeline with multi-platform testing

---

## Notes

**Priority Order:**

1. Phase 1-2: Essential for basic functionality
2. Phase 3-4: Required for automation
3. Phase 5: Critical for file management
4. Phase 6+: Enhancement and parity features

**Compatibility:**

- Maintain API compatibility with Lidarr v1.x where possible
- Document any breaking changes
- Provide migration guides for users

**Community:**

- Open to contributions after Phase 2 completion
- Maintain clear contributing guidelines
- Active issue tracking and triage

---

**Last Updated:** 2026-04-11  
**Current Phase:** Phase 6: User Interface Enhancement  
**Next Milestone:** Complete remaining download monitoring and client integration backlog items from Phase 4.2 / 4.3
