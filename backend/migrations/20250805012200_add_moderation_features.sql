-- Up Migration: Add moderation features for boards and bans

-- Create a new ENUM type for board moderation settings if it doesn't already exist.
-- 'alpha' is the default, traditional behavior (board owner moderates all).
-- 'beta' allows thread creators to moderate their own threads (future extension).
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'board_moderation_type') THEN
        CREATE TYPE board_moderation_type AS ENUM ('alpha', 'beta');
    END IF;
END$$;

-- Add the new moderation_type column to the boards table if it doesn't exist.
-- All existing boards will default to 'alpha'.
ALTER TABLE boards
ADD COLUMN IF NOT EXISTS moderation_type board_moderation_type NOT NULL DEFAULT 'alpha';

-- Add a post_id column to the bans table to support thread-specific bans if it doesn't exist.
-- This can be NULL, which is used for board-level or global bans.
ALTER TABLE bans
ADD COLUMN IF NOT EXISTS post_id INTEGER REFERENCES posts(id) ON DELETE SET NULL;

-- Drop the old 'is_global' boolean column if it exists. The new scope system
-- (based on NULL values in board_id and post_id) makes it redundant.
ALTER TABLE bans
DROP COLUMN IF EXISTS is_global;

-- Add an index on the new post_id column for performance if it doesn't exist.
CREATE INDEX IF NOT EXISTS idx_bans_post_id ON bans(post_id);

-- NOTE: The 'Down' migration section has been intentionally removed
-- to prevent accidental data loss from 'sqlx migrate revert' or 'cargo sqlx prepare'.
