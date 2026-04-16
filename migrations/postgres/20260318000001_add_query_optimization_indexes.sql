-- Add indexes for common repository lookup and filtered list patterns.
-- These target current repository hot-path queries without duplicating indexes
-- introduced by earlier migrations.

-- Artists: foreign ID lookups plus monitored/status lists ordered by name.
CREATE INDEX IF NOT EXISTS idx_artists_foreign_artist_id ON artists(foreign_artist_id);
CREATE INDEX IF NOT EXISTS idx_artists_monitored_name ON artists(monitored, name);
CREATE INDEX IF NOT EXISTS idx_artists_status_name ON artists(status, name);

-- Albums: foreign ID lookups, filtered lists, and upcoming release scans.
CREATE INDEX IF NOT EXISTS idx_albums_foreign_album_id ON albums(foreign_album_id);
CREATE INDEX IF NOT EXISTS idx_albums_status_title ON albums(status, title);
CREATE INDEX IF NOT EXISTS idx_albums_monitored_title ON albums(monitored, title);
CREATE INDEX IF NOT EXISTS idx_albums_album_type_title ON albums(album_type, title);
CREATE INDEX IF NOT EXISTS idx_albums_monitored_release_date_title ON albums(monitored, release_date, title);

-- Tracks: foreign ID lookups and filtered list scans.
CREATE INDEX IF NOT EXISTS idx_tracks_foreign_track_id ON tracks(foreign_track_id);
CREATE INDEX IF NOT EXISTS idx_tracks_monitored_track_number_title ON tracks(monitored, track_number, title);
CREATE INDEX IF NOT EXISTS idx_tracks_has_file_track_number_title ON tracks(has_file, track_number, title);

-- Track files: path lookups and recent/fingerprint processing scans.
DROP INDEX IF EXISTS idx_track_files_track_id;
DROP INDEX IF EXISTS idx_track_files_fingerprint_hash;
CREATE INDEX IF NOT EXISTS idx_track_files_path ON track_files(path);
CREATE INDEX IF NOT EXISTS idx_track_files_track_id_created_at ON track_files(track_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_track_files_created_at ON track_files(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_track_files_fingerprint_hash_created_at ON track_files(fingerprint_hash, created_at DESC);

-- Artist relationships: source/type queries ordered by recency.
CREATE INDEX IF NOT EXISTS idx_artist_relationships_source_type_created_at
    ON artist_relationships(source_artist_id, relationship_type, created_at DESC);
