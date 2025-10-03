-- Add migration script here
CREATE TABLE post_identities (
    id SERIAL PRIMARY KEY,
    post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    encrypted_email BYTEA, -- 暗号化したデータはバイナリで保存
    encrypted_ip BYTEA,
    encrypted_device_info BYTEA,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(post_id) -- 1投稿に1つのID情報
);