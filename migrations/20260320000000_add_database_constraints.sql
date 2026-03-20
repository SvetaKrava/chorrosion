-- Strengthen data integrity with DB-level constraints and uniqueness.
--
-- SQLite cannot add CHECK constraints to existing tables directly without
-- table rebuilds, so we enforce invariants using INSERT/UPDATE triggers.

-- Canonical external IDs should be unique when present.
CREATE UNIQUE INDEX IF NOT EXISTS ux_artists_musicbrainz_artist_id
ON artists(musicbrainz_artist_id)
WHERE musicbrainz_artist_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS ux_albums_musicbrainz_release_group_id
ON albums(musicbrainz_release_group_id)
WHERE musicbrainz_release_group_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS ux_albums_musicbrainz_release_id
ON albums(musicbrainz_release_id)
WHERE musicbrainz_release_id IS NOT NULL;

-- A file path should map to a single tracked file record.
-- Drop the old non-unique index first to avoid redundancy.
DROP INDEX IF EXISTS idx_track_files_path;

-- Preflight check: abort with a clear message if any duplicate paths exist
-- before we attempt to create the UNIQUE index on track_files(path).
-- SQLite's RAISE() requires a string literal (not an expression), so we check
-- for duplicates via a conditional INSERT and use a hardcoded message.
DROP TABLE IF EXISTS _migration_guard;
DROP TRIGGER IF EXISTS _migration_abort_on_dup_paths;

CREATE TABLE _migration_guard (ok INTEGER NOT NULL);

CREATE TRIGGER _migration_abort_on_dup_paths
BEFORE INSERT ON _migration_guard
WHEN NEW.ok = 0
BEGIN
  SELECT RAISE(ABORT, 'Duplicate track_files.path values found; resolve duplicates before this migration can create ux_track_files_path');
END;

-- Inserts a 0 (failing) row only when duplicates exist, triggering the ABORT above.
INSERT INTO _migration_guard (ok)
SELECT 0 WHERE EXISTS (
  SELECT 1 FROM track_files GROUP BY path HAVING COUNT(*) > 1
);

DROP TRIGGER IF EXISTS _migration_abort_on_dup_paths;
DROP TABLE IF EXISTS _migration_guard;

CREATE UNIQUE INDEX IF NOT EXISTS ux_track_files_path ON track_files(path);

-- Prevent self-referential artist relationships.
DROP TRIGGER IF EXISTS tr_artist_relationships_no_self_ref_insert;
CREATE TRIGGER tr_artist_relationships_no_self_ref_insert
BEFORE INSERT ON artist_relationships
FOR EACH ROW
WHEN NEW.source_artist_id = NEW.related_artist_id
BEGIN
  SELECT RAISE(ABORT, 'artist relationship cannot reference itself');
END;

DROP TRIGGER IF EXISTS tr_artist_relationships_no_self_ref_update;
CREATE TRIGGER tr_artist_relationships_no_self_ref_update
BEFORE UPDATE ON artist_relationships
FOR EACH ROW
WHEN NEW.source_artist_id = NEW.related_artist_id
BEGIN
  SELECT RAISE(ABORT, 'artist relationship cannot reference itself');
END;

-- Artists: valid status + non-empty names.
DROP TRIGGER IF EXISTS tr_artists_constraints_insert;
CREATE TRIGGER tr_artists_constraints_insert
BEFORE INSERT ON artists
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'artist name cannot be empty')
    WHERE trim(NEW.name) = '';
  SELECT RAISE(ABORT, 'invalid artist status')
    WHERE NEW.status NOT IN ('continuing', 'ended');
END;

DROP TRIGGER IF EXISTS tr_artists_constraints_update;
CREATE TRIGGER tr_artists_constraints_update
BEFORE UPDATE ON artists
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'artist name cannot be empty')
    WHERE trim(NEW.name) = '';
  SELECT RAISE(ABORT, 'invalid artist status')
    WHERE NEW.status NOT IN ('continuing', 'ended');
END;

-- Albums: valid status + non-empty titles.
DROP TRIGGER IF EXISTS tr_albums_constraints_insert;
CREATE TRIGGER tr_albums_constraints_insert
BEFORE INSERT ON albums
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'album title cannot be empty')
    WHERE trim(NEW.title) = '';
  SELECT RAISE(ABORT, 'invalid album status')
    WHERE NEW.status NOT IN ('wanted', 'released', 'announced');
END;

DROP TRIGGER IF EXISTS tr_albums_constraints_update;
CREATE TRIGGER tr_albums_constraints_update
BEFORE UPDATE ON albums
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'album title cannot be empty')
    WHERE trim(NEW.title) = '';
  SELECT RAISE(ABORT, 'invalid album status')
    WHERE NEW.status NOT IN ('wanted', 'released', 'announced');
END;

-- Tracks: non-empty title, positive optional numeric fields, bounded confidence.
DROP TRIGGER IF EXISTS tr_tracks_constraints_insert;
CREATE TRIGGER tr_tracks_constraints_insert
BEFORE INSERT ON tracks
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'track title cannot be empty')
    WHERE trim(NEW.title) = '';
  SELECT RAISE(ABORT, 'track_number must be >= 1')
    WHERE NEW.track_number IS NOT NULL AND NEW.track_number < 1;
  SELECT RAISE(ABORT, 'duration_ms must be > 0')
    WHERE NEW.duration_ms IS NOT NULL AND NEW.duration_ms <= 0;
  SELECT RAISE(ABORT, 'match_confidence must be between 0 and 1')
    WHERE NEW.match_confidence IS NOT NULL
      AND (NEW.match_confidence < 0 OR NEW.match_confidence > 1);
END;

DROP TRIGGER IF EXISTS tr_tracks_constraints_update;
CREATE TRIGGER tr_tracks_constraints_update
BEFORE UPDATE ON tracks
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'track title cannot be empty')
    WHERE trim(NEW.title) = '';
  SELECT RAISE(ABORT, 'track_number must be >= 1')
    WHERE NEW.track_number IS NOT NULL AND NEW.track_number < 1;
  SELECT RAISE(ABORT, 'duration_ms must be > 0')
    WHERE NEW.duration_ms IS NOT NULL AND NEW.duration_ms <= 0;
  SELECT RAISE(ABORT, 'match_confidence must be between 0 and 1')
    WHERE NEW.match_confidence IS NOT NULL
      AND (NEW.match_confidence < 0 OR NEW.match_confidence > 1);
END;

-- Track files: path non-empty and numeric fields constrained to valid ranges.
DROP TRIGGER IF EXISTS tr_track_files_constraints_insert;
CREATE TRIGGER tr_track_files_constraints_insert
BEFORE INSERT ON track_files
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'track file path cannot be empty')
    WHERE trim(NEW.path) = '';
  SELECT RAISE(ABORT, 'size_bytes must be >= 0')
    WHERE NEW.size_bytes < 0;
  SELECT RAISE(ABORT, 'duration_ms must be > 0')
    WHERE NEW.duration_ms IS NOT NULL AND NEW.duration_ms <= 0;
  SELECT RAISE(ABORT, 'bitrate_kbps must be > 0')
    WHERE NEW.bitrate_kbps IS NOT NULL AND NEW.bitrate_kbps <= 0;
  SELECT RAISE(ABORT, 'channels must be > 0')
    WHERE NEW.channels IS NOT NULL AND NEW.channels <= 0;
  SELECT RAISE(ABORT, 'fingerprint_duration must be > 0')
    WHERE NEW.fingerprint_duration IS NOT NULL AND NEW.fingerprint_duration <= 0;
END;

DROP TRIGGER IF EXISTS tr_track_files_constraints_update;
CREATE TRIGGER tr_track_files_constraints_update
BEFORE UPDATE ON track_files
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'track file path cannot be empty')
    WHERE trim(NEW.path) = '';
  SELECT RAISE(ABORT, 'size_bytes must be >= 0')
    WHERE NEW.size_bytes < 0;
  SELECT RAISE(ABORT, 'duration_ms must be > 0')
    WHERE NEW.duration_ms IS NOT NULL AND NEW.duration_ms <= 0;
  SELECT RAISE(ABORT, 'bitrate_kbps must be > 0')
    WHERE NEW.bitrate_kbps IS NOT NULL AND NEW.bitrate_kbps <= 0;
  SELECT RAISE(ABORT, 'channels must be > 0')
    WHERE NEW.channels IS NOT NULL AND NEW.channels <= 0;
  SELECT RAISE(ABORT, 'fingerprint_duration must be > 0')
    WHERE NEW.fingerprint_duration IS NOT NULL AND NEW.fingerprint_duration <= 0;
END;
