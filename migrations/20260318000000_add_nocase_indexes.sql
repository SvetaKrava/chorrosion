-- Add NOCASE indexes to allow efficient case-insensitive lookups on artist name
-- and album (artist_id, title), used by the list auto-add workflow.
CREATE INDEX IF NOT EXISTS idx_artists_name_nocase ON artists(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_albums_artist_id_title_nocase ON albums(artist_id, title COLLATE NOCASE);
