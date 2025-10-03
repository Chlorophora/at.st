-- Add up migration script here
-- We are changing from a hashed token to a raw, indexed token for efficient lookups.
-- The security of the session token comes from its randomness and length, not from being hashed in the DB.
ALTER TABLE sessions DROP COLUMN session_token_hash;
ALTER TABLE sessions ADD COLUMN session_token VARCHAR(128) NOT NULL;
CREATE UNIQUE INDEX sessions_session_token_idx ON sessions (session_token);