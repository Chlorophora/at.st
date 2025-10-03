-- 板のアーカイブ機能用のカラムを追加
ALTER TABLE boards ADD COLUMN last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
ALTER TABLE boards ADD COLUMN archived_at TIMESTAMPTZ;

-- 板作成者のIPアドレスとデバイス情報を保存するためのテーブルを作成
CREATE TABLE board_identities (
    id SERIAL PRIMARY KEY,
    board_id INTEGER NOT NULL UNIQUE REFERENCES boards(id) ON DELETE CASCADE,
    encrypted_ip TEXT,
    encrypted_device_info TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);