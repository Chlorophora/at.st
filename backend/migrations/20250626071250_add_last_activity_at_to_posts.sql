-- Add migration script here
ALTER TABLE posts ADD COLUMN last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW();