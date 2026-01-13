-- Add genre and style tag fields for comprehensive metadata categorization

-- Artists: Add genre and style tags for music categorization
ALTER TABLE artists ADD COLUMN genre_tags TEXT;
ALTER TABLE artists ADD COLUMN style_tags TEXT;

-- Albums: Add genre and style tags for music categorization
ALTER TABLE albums ADD COLUMN genre_tags TEXT;
ALTER TABLE albums ADD COLUMN style_tags TEXT;

-- Create indices for efficient tag-based queries and filtering
-- Note: Genre/style tags are stored as pipe-delimited strings for flexible querying
-- Example: "rock|alternative|indie" or "aggressive|energetic"
CREATE INDEX IF NOT EXISTS idx_artists_genre_tags ON artists(genre_tags);
CREATE INDEX IF NOT EXISTS idx_artists_style_tags ON artists(style_tags);
CREATE INDEX IF NOT EXISTS idx_albums_genre_tags ON albums(genre_tags);
CREATE INDEX IF NOT EXISTS idx_albums_style_tags ON albums(style_tags);
