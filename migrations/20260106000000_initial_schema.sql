-- Create artists table
CREATE TABLE IF NOT EXISTS artists (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  foreign_artist_id TEXT,
  metadata_profile_id TEXT,
  quality_profile_id TEXT,
  status TEXT NOT NULL DEFAULT 'continuing',
  path TEXT,
  monitored BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_artists_name ON artists(name);

-- Create albums table
CREATE TABLE IF NOT EXISTS albums (
  id TEXT PRIMARY KEY,
  artist_id TEXT NOT NULL,
  foreign_album_id TEXT,
  title TEXT NOT NULL,
  release_date DATE,
  album_type TEXT,
  status TEXT NOT NULL DEFAULT 'wanted',
  monitored BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE CASCADE
);

CREATE INDEX idx_albums_artist_id ON albums(artist_id);
CREATE INDEX idx_albums_title ON albums(title);

-- Create tracks table
CREATE TABLE IF NOT EXISTS tracks (
  id TEXT PRIMARY KEY,
  album_id TEXT NOT NULL,
  artist_id TEXT NOT NULL,
  foreign_track_id TEXT,
  title TEXT NOT NULL,
  track_number INTEGER,
  duration_ms INTEGER,
  has_file BOOLEAN NOT NULL DEFAULT FALSE,
  monitored BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (album_id) REFERENCES albums(id) ON DELETE CASCADE,
  FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE CASCADE
);

CREATE INDEX idx_tracks_album_id ON tracks(album_id);
CREATE INDEX idx_tracks_artist_id ON tracks(artist_id);

-- Create quality profiles table
CREATE TABLE IF NOT EXISTS quality_profiles (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  allowed_qualities TEXT NOT NULL,
  upgrade_allowed BOOLEAN NOT NULL DEFAULT FALSE,
  cutoff_quality TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create metadata profiles table
CREATE TABLE IF NOT EXISTS metadata_profiles (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  primary_album_types TEXT,
  secondary_album_types TEXT,
  release_statuses TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create job log table for background tasks
CREATE TABLE IF NOT EXISTS job_logs (
  id TEXT PRIMARY KEY,
  job_type TEXT NOT NULL,
  status TEXT NOT NULL,
  message TEXT,
  started_at TIMESTAMP NOT NULL,
  completed_at TIMESTAMP,
  error_message TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_job_logs_job_type ON job_logs(job_type);
CREATE INDEX idx_job_logs_status ON job_logs(status);
CREATE INDEX idx_job_logs_started_at ON job_logs(started_at);
