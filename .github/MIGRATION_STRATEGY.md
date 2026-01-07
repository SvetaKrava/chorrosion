# Migration Strategy & Future Plans

## Database Migration Tool (Future)

### Overview

A separate migration utility will allow users to import data from an existing Lidarr (C#/.NET) instance into Chorrosion without requiring direct database schema compatibility.

### Rationale

- **Schema Independence**: Chorrosion can evolve and improve its schema independently of the original Lidarr design.
- **Clean Start**: Users get a fresh, optimized schema without legacy baggage.
- **Controlled Migration**: Transforms data intelligently during import (cleanup, deduplication, optimization).
- **Future-Proof**: Allows schema versioning and evolution without breaking migrations.

### High-Level Design

#### Data Flow

```txt
Original Lidarr DB
    ↓ (read via SQL)
Transform & Validate Layer
    ↓ (map, deduplicate, validate)
Chorrosion Schema
    ↓ (insert)
New Database Instance
```

#### Planned Features

1. **Artist & Album Import**
   - Map artists and their metadata (name, foreign ID, monitored status)
   - Preserve albums, tracks, and release information
   - Transform quality/metadata profiles if needed

2. **Profile Migration**
   - Quality profiles (allowed qualities, cutoff, upgrade allowed)
   - Metadata profiles (album types, release statuses)
   - Handle profile remapping if schemas differ

3. **File Tracking**
   - Import `TrackFiles` data to populate `tracks.has_file` flag
   - Optionally preserve file paths for reimport workflows

4. **Validation & Reporting**
   - Pre-flight checks: validate source DB connectivity and integrity
   - Generate migration report: counts, skipped records, errors
   - Post-import validation: ensure referential integrity

5. **Rollback & Idempotency**
   - Dry-run mode to preview changes
   - Transaction support for atomic imports
   - Handle duplicate runs gracefully

#### Implementation Considerations

- **Tool Type**: Standalone CLI binary (maybe `chorrosion-migrate` crate or separate binary in workspace)
- **Connection Methods**:
  - Direct SQLite/PostgreSQL connection to source Lidarr DB
  - Potentially API-based export if DB access isn't available
- **Error Handling**: Partial imports with detailed logging; allow resume/skip
- **Testing**: Fixtures for known Lidarr database versions

#### Known Source Lidarr Tables to Handle

- `Artists`
- `Albums`
- `Tracks`
- `TrackFiles`
- `QualityProfiles`
- `MetadataProfiles`
- `ReleaseProfiles` (if applicable)

### Next Steps

1. Document exact schema mapping between original Lidarr and Chorrosion
2. Prototype a read-only connection to sample Lidarr DB to understand data patterns
3. Build skeleton `chorrosion-migrate` crate with trait-based adapters for different source versions
4. Create end-to-end test with sample fixtures

### Related Issues/PRs

(To be updated as work progresses)
