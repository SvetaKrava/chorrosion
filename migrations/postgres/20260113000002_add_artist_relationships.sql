-- Add artist relationships table for tracking collaborations and connections

-- Create artist_relationships table for storing connections between artists
CREATE TABLE IF NOT EXISTS artist_relationships (
  id TEXT PRIMARY KEY,
  source_artist_id TEXT NOT NULL,
  related_artist_id TEXT NOT NULL,
  relationship_type TEXT NOT NULL,
  description TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (source_artist_id) REFERENCES artists(id) ON DELETE CASCADE,
  FOREIGN KEY (related_artist_id) REFERENCES artists(id) ON DELETE CASCADE,
  UNIQUE(source_artist_id, related_artist_id, relationship_type)
);

-- Create indices for efficient relationship queries
CREATE INDEX IF NOT EXISTS idx_artist_relationships_source_id ON artist_relationships(source_artist_id);
CREATE INDEX IF NOT EXISTS idx_artist_relationships_related_id ON artist_relationships(related_artist_id);
CREATE INDEX IF NOT EXISTS idx_artist_relationships_type ON artist_relationships(relationship_type);

-- Relationship types supported:
-- 'collaborator' - artists who have worked together
-- 'member' - member of a group
-- 'featuring' - featured artist on a recording
-- 'remix_artist' - remixed work by this artist
-- 'influences' - influenced by this artist
-- 'similar' - similar artistic style
-- 'related' - generic relationship
