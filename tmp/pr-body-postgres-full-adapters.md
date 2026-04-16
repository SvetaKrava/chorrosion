## Summary

Implements complete `Repository<T>` and specialized trait methods for all 8 remaining PostgreSQL adapter structs introduced in #383. All adapters were previously scaffolded (struct + `new()`/`pool()` only); this PR fills in every CRUD operation plus domain-specific queries.

## Adapters Implemented

- `PostgresAlbumRepository` → `Repository<Album>` + `AlbumRepository`
- `PostgresTrackRepository` → `Repository<Track>` + `TrackRepository`
- `PostgresQualityProfileRepository` → `Repository<QualityProfile>` + `QualityProfileRepository`
- `PostgresMetadataProfileRepository` → `Repository<MetadataProfile>` + `MetadataProfileRepository`
- `PostgresIndexerDefinitionRepository` → `Repository<IndexerDefinition>` + `IndexerDefinitionRepository`
- `PostgresDownloadClientDefinitionRepository` → `Repository<DownloadClientDefinition>` + `DownloadClientDefinitionRepository`
- `PostgresTrackFileRepository` → `Repository<TrackFile>` + `TrackFileRepository`
- `PostgresArtistRelationshipRepository` → `Repository<ArtistRelationship>` + `ArtistRelationshipRepository`

## Postgres-Specific Adaptations

- **Timestamps**: read as `NaiveDateTime` (native Postgres type), converted to `DateTime<Utc>`
- **JSON columns** (`allowed_qualities`, `primary_album_types`, etc.): stored as TEXT, serialized with `serde_json`
- **Type coercions**: `size_bytes` bound as `i64`, `channels` as `i16`, `track_number`/`duration_ms` as `i32`
- **Boolean filters**: `monitored = true` / `has_file = false` (not `= 1`)
- **`list_cutoff_unmet_albums`**: uses `jsonb_array_elements_text()` instead of SQLite's `json_each()`
- **Case-insensitive title search**: `ILIKE` instead of `LIKE`

## Testing

- `cargo build -p chorrosion-infrastructure --features postgres` ✅
- `cargo clippy -p chorrosion-infrastructure --features postgres -- -D warnings` ✅
- `cargo test -p chorrosion-infrastructure` — 84 tests pass ✅

Closes part of #12
Builds on #383
