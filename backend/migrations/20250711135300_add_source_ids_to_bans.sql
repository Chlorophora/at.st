-- Add migration script here
-- Add columns to track the source of a ban
ALTER TABLE bans
ADD COLUMN source_post_id INTEGER REFERENCES posts(id) ON DELETE SET NULL,
ADD COLUMN source_comment_id INTEGER REFERENCES comments(id) ON DELETE SET NULL;

-- Add indexes for faster lookups
CREATE INDEX IF NOT EXISTS idx_bans_source_post_id ON bans(source_post_id);
CREATE INDEX IF NOT EXISTS idx_bans_source_comment_id ON bans(source_comment_id);