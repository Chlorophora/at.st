-- Add up migration script here
ALTER TABLE comments ADD COLUMN user_id INTEGER REFERENCES users(id) ON DELETE SET NULL;