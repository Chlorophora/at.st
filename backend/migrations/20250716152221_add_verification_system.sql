-- A-- migrations/YYYYMMDDHHMMSS_add_verification_system.sql

-- 1. 検証試行の履歴を保存する `level_up_attempts` テーブルを新設します。
-- このテーブルは、レベル上げとアカウント作成の両方の試行を記録するためのものです。
-- これにより、「どのIP/フィンガープリントがいつ成功したか」を追跡し、
-- 一定期間の再利用を禁止するルールの判定に用います。
CREATE TABLE level_up_attempts (
    -- 基本的なレコード識別子
    id SERIAL PRIMARY KEY,
    -- どのユーザーが試行したか。アカウント作成時はまだユーザーIDがないためNULLを許容します。
    user_id INTEGER REFERENCES users(id),
    -- この試行が 'level_up'（レベル上げ）なのか 'registration'（アカウント作成）なのかを区別します。
    attempt_type VARCHAR(20) NOT NULL,
    -- 試行が行われた日時
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- 検証に成功したか(true)、失敗したか(false)
    is_success BOOLEAN NOT NULL,
    -- 試行時に使用されたIPアドレス
    ip_address VARCHAR(45),
    -- SPURから取得した一意のID。このIDの再利用をチェックするために使います。
    spur_monocle_id VARCHAR(255),
    -- SPURから返却された全情報をJSON形式で保存します。調査や分析に役立ちます。
    spur_monocle_json JSONB,
    -- FingerprintJSから収集した全情報をJSON形式で保存します。
    fingerprint_json JSONB,
    -- 「webgl + canvas + audio」の3要素をまとめたハッシュ。23時間ロックの判定に使います。
    hash_webgl_canvas_audio VARCHAR(64),
    -- 「webgl + canvas」の2要素をまとめたハッシュ。1時間ロックの判定に使います。
    hash_webgl_canvas VARCHAR(64),
    -- 「webgl + audio」の2要素をまとめたハッシュ。1時間ロックの判定に使います。
    hash_webgl_audio VARCHAR(64),
    -- 「canvas + audio」の2要素をまとめたハッシュ。1時間ロックの判定に使います。
    hash_canvas_audio VARCHAR(64),
    -- 検証に失敗した場合、その理由を記録します。（例: 'Region not allowed.')
    rejection_reason TEXT
);

-- 検索パフォーマンス向上のため、頻繁に検索条件として使われるカラムにインデックスを作成します。
CREATE INDEX idx_level_up_attempts_user_id ON level_up_attempts(user_id);
CREATE INDEX idx_level_up_attempts_success_time ON level_up_attempts(is_success, created_at);
CREATE INDEX idx_level_up_attempts_spur ON level_up_attempts(spur_monocle_id, ip_address);
CREATE INDEX idx_level_up_attempts_h3 ON level_up_attempts(hash_webgl_canvas_audio);
CREATE INDEX idx_level_up_attempts_h2 ON level_up_attempts(hash_webgl_canvas, hash_webgl_audio, hash_canvas_audio);


-- 2. `users` テーブルを修正し、レート制限と禁止ステータスを管理します。
-- 既存の `last_level_up_fingerprint` は、より詳細な `level_up_attempts` テーブルに
-- 役割を移行するため、このタイミングで削除します。
ALTER TABLE users DROP COLUMN IF EXISTS last_level_up_fingerprint;

-- レート制限と禁止ステータス管理用のカラムを追加します。
-- レベル上げに連続で失敗した回数を記録します。5回に達したらロックをかける判定に使います。
ALTER TABLE users ADD COLUMN level_up_failure_count INTEGER NOT NULL DEFAULT 0;
-- 成功・失敗を問わず、最後にレベル上げを「試行」した日時。
ALTER TABLE users ADD COLUMN last_level_up_attempt_at TIMESTAMPTZ;
-- 管理者が手動で特定のユーザーのレベル上げを禁止するためのフラグ。
ALTER TABLE users ADD COLUMN banned_from_level_up BOOLEAN NOT NULL DEFAULT FALSE;

-- 補足: 既存のカラム `last_level_up_at` の役割が少し変わります。
-- 今後は「レベル上げに成功した日時」または「5回失敗して23時間のロックがかかった日時」を
-- 記録するために使用されるようになります。