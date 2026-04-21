-- Create tags table for user-defined tag management
CREATE TABLE IF NOT EXISTS tags (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL COLLATE NOCASE UNIQUE,
  description TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create tagged_entities table for mapping tags to artists and albums
CREATE TABLE IF NOT EXISTS tagged_entities (
  tag_id TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (tag_id, entity_id, entity_type),
  FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE INDEX idx_tagged_entities_entity ON tagged_entities(entity_id, entity_type);
CREATE INDEX idx_tagged_entities_tag_id ON tagged_entities(tag_id);

CREATE TRIGGER IF NOT EXISTS trg_tagged_entities_cleanup_artist
AFTER DELETE ON artists
BEGIN
  DELETE FROM tagged_entities WHERE entity_id = OLD.id AND entity_type = 'artist';
END;

CREATE TRIGGER IF NOT EXISTS trg_tagged_entities_cleanup_album
AFTER DELETE ON albums
BEGIN
  DELETE FROM tagged_entities WHERE entity_id = OLD.id AND entity_type = 'album';
END;
