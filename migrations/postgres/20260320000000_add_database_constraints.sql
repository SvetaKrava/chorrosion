-- Strengthen data integrity with Postgres-native constraints and unique indexes.

-- Canonical external IDs should be unique when present.
CREATE UNIQUE INDEX IF NOT EXISTS ux_artists_musicbrainz_artist_id
ON artists(musicbrainz_artist_id)
WHERE musicbrainz_artist_id IS NOT NULL;
DROP INDEX IF EXISTS idx_artists_musicbrainz_id;

CREATE UNIQUE INDEX IF NOT EXISTS ux_albums_musicbrainz_release_group_id
ON albums(musicbrainz_release_group_id)
WHERE musicbrainz_release_group_id IS NOT NULL;
DROP INDEX IF EXISTS idx_albums_musicbrainz_release_group_id;

CREATE UNIQUE INDEX IF NOT EXISTS ux_albums_musicbrainz_release_id
ON albums(musicbrainz_release_id)
WHERE musicbrainz_release_id IS NOT NULL;
DROP INDEX IF EXISTS idx_albums_musicbrainz_release_id;

-- A file path should map to a single tracked file record.
DROP INDEX IF EXISTS idx_track_files_path;
CREATE UNIQUE INDEX IF NOT EXISTS ux_track_files_path ON track_files(path);

-- Prevent self-referential artist relationships.
ALTER TABLE artist_relationships
  ADD CONSTRAINT ck_artist_relationships_no_self_ref
  CHECK (source_artist_id <> related_artist_id);

-- Artists: valid status + non-empty names.
ALTER TABLE artists
  ADD CONSTRAINT ck_artists_name_non_empty
  CHECK (btrim(name) <> '');

ALTER TABLE artists
  ADD CONSTRAINT ck_artists_valid_status
  CHECK (status IN ('continuing', 'ended'));

-- Albums: valid status + non-empty titles.
ALTER TABLE albums
  ADD CONSTRAINT ck_albums_title_non_empty
  CHECK (btrim(title) <> '');

ALTER TABLE albums
  ADD CONSTRAINT ck_albums_valid_status
  CHECK (status IN ('wanted', 'released', 'announced'));

-- Tracks: non-empty title, positive optional numeric fields, bounded confidence.
ALTER TABLE tracks
  ADD CONSTRAINT ck_tracks_title_non_empty
  CHECK (btrim(title) <> '');

ALTER TABLE tracks
  ADD CONSTRAINT ck_tracks_track_number_positive
  CHECK (track_number IS NULL OR track_number >= 1);

ALTER TABLE tracks
  ADD CONSTRAINT ck_tracks_duration_positive
  CHECK (duration_ms IS NULL OR duration_ms > 0);

ALTER TABLE tracks
  ADD CONSTRAINT ck_tracks_match_confidence_bounded
  CHECK (match_confidence IS NULL OR (match_confidence >= 0 AND match_confidence <= 1));

-- Track files: path non-empty and numeric fields constrained to valid ranges.
ALTER TABLE track_files
  ADD CONSTRAINT ck_track_files_path_non_empty
  CHECK (btrim(path) <> '');

ALTER TABLE track_files
  ADD CONSTRAINT ck_track_files_size_non_negative
  CHECK (size_bytes >= 0);

ALTER TABLE track_files
  ADD CONSTRAINT ck_track_files_duration_positive
  CHECK (duration_ms IS NULL OR duration_ms > 0);

ALTER TABLE track_files
  ADD CONSTRAINT ck_track_files_bitrate_positive
  CHECK (bitrate_kbps IS NULL OR bitrate_kbps > 0);

ALTER TABLE track_files
  ADD CONSTRAINT ck_track_files_channels_positive
  CHECK (channels IS NULL OR channels > 0);

ALTER TABLE track_files
  ADD CONSTRAINT ck_track_files_fingerprint_duration_positive
  CHECK (fingerprint_duration IS NULL OR fingerprint_duration > 0);
