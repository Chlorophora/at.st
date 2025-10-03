-- Add migration script here

-- BANの種類を定義するENUM型を作成
CREATE TYPE ban_type AS ENUM ('user', 'ip', 'device');

-- BAN情報を格納するテーブルを作成
CREATE TABLE bans (
    id SERIAL PRIMARY KEY,
    ban_type ban_type NOT NULL,
    hash_value TEXT NOT NULL,
    board_id INTEGER REFERENCES boards(id), -- NULLの場合はグローバルBAN
    reason TEXT,
    created_by INTEGER NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ, -- NULLの場合は無期限BAN

    -- 同じスコープ(board_id)で同じ種類のハッシュが重複しないように制約
    -- グローバルBAN(board_id IS NULL)の場合も考慮
    CONSTRAINT unique_ban UNIQUE (ban_type, hash_value, board_id)
);

-- 検索を高速化するためのインデックスを作成
CREATE INDEX idx_bans_on_hash_value ON bans(hash_value);
CREATE INDEX idx_bans_on_board_id ON bans(board_id);
