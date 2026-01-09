-- Create track_files table if it doesn't exist
CREATE TABLE IF NOT EXISTS track_files (
  id TEXT PRIMARY KEY,
  track_id TEXT NOT NULL,
  path TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  duration_ms INTEGER,
  bitrate_kbps INTEGER,
  channels INTEGER,
  codec TEXT,
  hash TEXT,
  fingerprint_hash TEXT,
  fingerprint_duration INTEGER,
  fingerprint_computed_at TIMESTAMP,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_track_files_track_id ON track_files(track_id);
CREATE INDEX IF NOT EXISTS idx_track_files_fingerprint_hash ON track_files(fingerprint_hash);

-- Add recording metadata to tracks table
ALTER TABLE tracks ADD COLUMN musicbrainz_recording_id TEXT;
ALTER TABLE tracks ADD COLUMN match_confidence REAL;

-- Create index for recording lookups
CREATE INDEX IF NOT EXISTS idx_tracks_recording_id ON tracks(musicbrainz_recording_id);
