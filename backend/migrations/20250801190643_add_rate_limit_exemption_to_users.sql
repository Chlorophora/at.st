-- Add migration script here
ALTER TABLE users
ADD COLUMN is_rate_limit_exempt BOOLEAN NOT NULL DEFAULT FALSE;
