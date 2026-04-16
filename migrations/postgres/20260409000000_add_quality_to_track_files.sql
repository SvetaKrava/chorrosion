-- Add quality label column to track_files.
-- This stores the resolved quality string (e.g. "FLAC", "MP3 320") that maps
-- to an entry in the artist's QualityProfile.allowed_qualities list, enabling
-- profile-based upgrade decisions and cutoff evaluation.
ALTER TABLE track_files ADD COLUMN IF NOT EXISTS quality TEXT;
