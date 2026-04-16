CREATE TABLE IF NOT EXISTS download_client_definitions (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  client_type TEXT NOT NULL,
  base_url TEXT NOT NULL,
  username TEXT,
  password_encrypted TEXT, -- TODO: encrypt before storing; currently stored in plaintext
  category TEXT,
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_download_client_definitions_enabled ON download_client_definitions(enabled);
