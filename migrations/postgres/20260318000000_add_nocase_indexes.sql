-- Add case-insensitive indexes used by list auto-add and lookups.
CREATE INDEX IF NOT EXISTS idx_artists_name_lower ON artists (LOWER(name));
CREATE INDEX IF NOT EXISTS idx_albums_artist_id_title_lower ON albums (artist_id, LOWER(title));
