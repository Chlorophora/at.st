-- レート制限の監視対象を定義するENUM型
CREATE TYPE rate_limit_target_type AS ENUM (
    'UserId',
    'IpAddress',
    'DeviceId',
    'UserAndIp',
    'UserAndDevice',
    'IpAndDevice',
    'All'
);

-- 管理者が設定するレート制限ルールを保存するテーブル
CREATE TABLE rate_limit_rules (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    target rate_limit_target_type NOT NULL,
    threshold INTEGER NOT NULL CHECK (threshold > 0),
    time_frame_seconds INTEGER NOT NULL CHECK (time_frame_seconds > 0),
    lockout_seconds INTEGER NOT NULL CHECK (lockout_seconds > 0),
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by INTEGER NOT NULL REFERENCES users(id)
);

-- 投稿イベントを一時的に記録するテーブル
-- このテーブルのデータは定期的にクリーンアップされる
CREATE TABLE rate_limit_tracker (
    rule_id INTEGER NOT NULL REFERENCES rate_limit_rules(id) ON DELETE CASCADE,
    target_key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- パフォーマンス向上のためのインデックス
CREATE INDEX idx_rate_limit_tracker_key_time ON rate_limit_tracker (rule_id, target_key, created_at);
CREATE INDEX idx_rate_limit_tracker_created_at ON rate_limit_tracker (created_at);

-- 現在レート制限によりロックされている対象を保存するテーブル
-- このテーブルのデータは定期的にクリーンアップされる
CREATE TABLE rate_limit_locks (
    target_key TEXT PRIMARY KEY,
    rule_id INTEGER NOT NULL REFERENCES rate_limit_rules(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL
);

-- パフォーマンス向上のためのインデックス
CREATE INDEX idx_rate_limit_locks_expires_at ON rate_limit_locks (expires_at);
