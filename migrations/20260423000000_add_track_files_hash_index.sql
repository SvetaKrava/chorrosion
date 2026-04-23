CREATE INDEX IF NOT EXISTS idx_track_files_hash_created_at
    ON track_files(hash, created_at DESC);
