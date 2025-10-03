-- Add migration script here
CREATE TABLE comment_identities (
    id SERIAL PRIMARY KEY,
    comment_id INTEGER NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
    encrypted_email BYTEA,
    encrypted_ip BYTEA,
    encrypted_device_info BYTEA,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(comment_id)
);