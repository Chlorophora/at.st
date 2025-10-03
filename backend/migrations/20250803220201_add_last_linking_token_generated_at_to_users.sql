-- Add last_linking_token_generated_at to users table
ALTER TABLE users ADD COLUMN last_linking_token_generated_at TIMESTAMPTZ;