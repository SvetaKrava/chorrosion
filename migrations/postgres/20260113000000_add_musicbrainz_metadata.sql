-- Add MusicBrainz metadata fields for comprehensive metadata storage

-- Artists: Add proper MBID and rich metadata fields
ALTER TABLE artists ADD COLUMN IF NOT EXISTS musicbrainz_artist_id TEXT;
ALTER TABLE artists ADD COLUMN IF NOT EXISTS artist_type TEXT;
ALTER TABLE artists ADD COLUMN IF NOT EXISTS sort_name TEXT;
ALTER TABLE artists ADD COLUMN IF NOT EXISTS country TEXT;
ALTER TABLE artists ADD COLUMN IF NOT EXISTS disambiguation TEXT;

-- Create index for MBID lookup
CREATE INDEX IF NOT EXISTS idx_artists_musicbrainz_id ON artists(musicbrainz_artist_id);

-- Albums: Add proper MBID and rich metadata fields
ALTER TABLE albums ADD COLUMN IF NOT EXISTS musicbrainz_release_group_id TEXT;
ALTER TABLE albums ADD COLUMN IF NOT EXISTS musicbrainz_release_id TEXT;
ALTER TABLE albums ADD COLUMN IF NOT EXISTS primary_type TEXT;
ALTER TABLE albums ADD COLUMN IF NOT EXISTS secondary_types TEXT;
ALTER TABLE albums ADD COLUMN IF NOT EXISTS first_release_date TEXT;

-- Create indices for MBID lookups
CREATE INDEX IF NOT EXISTS idx_albums_musicbrainz_release_group_id ON albums(musicbrainz_release_group_id);
CREATE INDEX IF NOT EXISTS idx_albums_musicbrainz_release_id ON albums(musicbrainz_release_id);

-- Tracks: musicbrainz_recording_id already exists from previous migration
-- Add any additional metadata fields if needed in future migrations
