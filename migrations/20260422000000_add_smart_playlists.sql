-- SPDX-License-Identifier: GPL-3.0-or-later

CREATE TABLE IF NOT EXISTS smart_playlists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    description TEXT,
    criteria_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_smart_playlists_name ON smart_playlists(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_smart_playlists_updated_at ON smart_playlists(updated_at DESC);
