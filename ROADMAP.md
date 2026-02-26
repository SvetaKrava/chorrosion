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

- [ ] Manual search implementation
  - [ ] Artist search
  - [ ] Album search
  - [ ] Interactive search UI support
- [ ] Automatic search
  - [ ] Missing album detection
  - [ ] Search triggering logic
  - [ ] Best release selection algorithm
- [ ] RSS sync enhancement
  - [ ] Parse RSS feeds from indexers
  - [ ] Match releases to wanted albums
  - [ ] Automatic grab logic

### 4.2 Download Client Integration

- [ ] Download client trait definition
- [ ] qBittorrent client
  - [ ] Add torrent support
  - [ ] Category management
  - [ ] Status monitoring
- [ ] Transmission client
- [ ] Deluge client
- [ ] SABnzbd client (Usenet)
- [ ] NZBGet client (Usenet)
- [ ] Download queue management

### 4.3 Download Monitoring

- [ ] Download status tracking
- [ ] Completion detection
- [ ] Failed download handling
- [ ] Stalled download detection
- [ ] Download history

---

## Phase 5: File Management (Q3 2026)

### 5.1 Import System

- [ ] File scanning and detection
- [ ] Track file parsing (tags, duration, bitrate)
- [x] **Fingerprint generation during import** (Issue #67) ✓
  - [x] Generate Chromaprint fingerprint ✓
  - [x] Cache in database (Issue #66) ✓
  - [x] Store in TrackFile domain model ✓
- [ ] File matching algorithm
  - [x] **Primary: Fingerprint-based lookup** (Issue #68) ✓
    - [x] Query AcoustID with fingerprint ✓
    - [x] Link to MusicBrainz recording ✓
    - [ ] Link to artist/album via recording
  - [x] Fallback: Embedded tag matching (Issue #28) ✓
  - [x] Fallback: Filename heuristics (Issue #28) ✓
  - [ ] Fuzzy matching for poor metadata
- [ ] Import decision logic
- [ ] Manual import UI support

### 5.2 File Organization

- [ ] File renaming implementation
  - [ ] File Path Generation and Naming Logic (Issue #21)
  - [ ] Naming pattern engine
  - [ ] Token replacement (artist, album, track, etc.)
  - [ ] Safe file operations
- [ ] Folder organization
  - [ ] Artist folder structure
  - [ ] Album folder structure
  - [ ] Multi-disc handling
- [ ] File operations
  - [ ] Copy vs. move logic
  - [ ] Hard link support
  - [ ] Permission handling

### 5.3 Quality Management

- [ ] Quality upgrades detection
  - [ ] Compare existing vs. new quality
  - [ ] Upgrade decision logic
  - [ ] Cutoff management
- [ ] File replacement workflow
- [ ] Backup of replaced files (optional)

### 5.4 Tagging & Embedding

_(See design: [Embedded Tags Behavior](DESIGN.md#embedded-tags-behavior))_

- [ ] Embed metadata and artwork in supported formats
  - [ ] ID3v2 (MP3): tags + front cover artwork
  - [ ] Vorbis Comments (FLAC/OGG): tags + embedded pictures + fingerprint
  - [ ] MP4/M4A atoms: tags + cover art (`covr`) + fingerprint
  - [ ] Safe, atomic writes with backup/rollback on failure
  - [ ] Charset/normalization handling and tag sanitation
  - [ ] Configurable per profile (enable/disable, overwrite rules)
  - [ ] User preference: preserve embedded metadata/art (no writes) vs overwrite on import/refresh
  - [ ] Read-only tag mode that never modifies source files
  - [ ] Fallback behavior for unsupported file types
  - [ ] **Store computed fingerprint in file tags** (Phase 5.4 follow-up)

---

## Phase 6: User Interface Enhancement (Q3-Q4 2026)

- Related issues: #9, #10, #11

### 6.1 API Completion

- [ ] Complete all artist endpoints
  - [ ] List with pagination and sorting
  - [ ] Detailed artist view
  - [ ] Update monitored status
  - [ ] Artist statistics
- [ ] Album endpoints
  - [ ] List by artist
  - [ ] Album details
  - [ ] Monitor toggle
  - [ ] Search trigger
- [ ] Track endpoints
- [ ] Queue/Activity endpoints
  - [ ] Download queue
  - [ ] History
  - [ ] Currently processing
- [ ] System endpoints
  - [ ] Status and version
  - [ ] Tasks/jobs management
  - [ ] Log viewing
- [ ] Settings endpoints
  - [ ] Quality profiles CRUD
  - [ ] Metadata profiles CRUD
  - [ ] Indexer management
  - [ ] Download client management

### 6.2 WebSocket/SSE Support

- [ ] Real-time updates implementation
  - [ ] Download progress
  - [ ] Import progress
  - [ ] Job status
- [ ] Event broadcasting
- [ ] Client connection management

### 6.3 Authentication & Authorization

- [ ] API key generation and management
- [ ] Basic authentication support
- [ ] Forms authentication (optional)
- [ ] Permission levels (optional)

---

## Phase 7: Advanced Features (Q4 2026)

### 7.1 Wanted/Missing Management

- [ ] Wanted album tracking
- [ ] Missing album detection
- [ ] Cutoff unmet detection
- [ ] Automated search scheduling
- [ ] Manual search interface

### 7.2 Calendar

- [ ] Upcoming releases calendar
- [ ] Release date tracking
  - [ ] Calendar API endpoints
  - [ ] iCal feed support

### 7.3 Notifications

- [ ] Notification framework
- [ ] Email notifications
- [ ] Discord webhook
- [ ] Slack webhook
- [ ] Pushover integration
- [ ] Custom scripts support

### 7.4 Lists Integration

- [ ] List provider trait
- [ ] MusicBrainz list import
- [ ] Spotify playlist import (optional)
- [ ] Last.fm integration
- [ ] Auto-add from lists

---

## Phase 8: Performance & Reliability (Ongoing)

- Related issues: #13, #37, #38, #39, #40, #41, #42

### 8.1 Performance Optimization

- [ ] Database query optimization
  - [ ] Add indexes for common queries
  - [ ] Query profiling and tuning
- [ ] Caching layer
  - [ ] Metadata caching
  - [ ] API response caching
  - [ ] File system cache
- [ ] Concurrent operation improvements
- [ ] Memory usage optimization

### 8.2 Reliability

- [ ] Comprehensive error handling
  - [ ] Retry logic for external APIs
  - [ ] Timeout handling
  - [ ] Rate limit handling
- [ ] Data integrity
  - [ ] Database constraints
  - [ ] Transaction management
  - [ ] Backup/restore functionality
- [ ] Monitoring and observability
  - [ ] Metrics collection (Prometheus)
  - [ ] Health checks
  - [ ] Performance tracing

### 8.3 Testing

- [ ] Unit test coverage
  - [ ] Repository tests
  - [ ] Business logic tests
  - [ ] API endpoint tests
- [ ] Integration tests
  - [ ] Database integration tests
  - [ ] External API mock tests
- [ ] End-to-end tests
  - [ ] Full workflow tests
  - [ ] Performance benchmarks

### 8.4 Maintenance & Dependencies

- [ ] Address future-incompat warnings in dependencies
  - [x] Upgrade `sqlx`/`sqlx-postgres` to 0.8.x or newer to resolve never-type fallback warnings
  - [ ] Gate PostgreSQL behind a feature and disable by default to avoid pulling incompatible crates until upgraded
  - [ ] Add a CI job to run `cargo report future-incompatibilities --id 2` and fail on new findings
  - [ ] Track Rust 2024 edition changes (e.g., never type fallback) and ensure readiness before edition bump

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

**Last Updated:** 2026-02-26  
**Current Phase:** Phase 4: Search & Download  
**Next Milestone:** Manual and automatic search implementation kickoff (Issue #33)
