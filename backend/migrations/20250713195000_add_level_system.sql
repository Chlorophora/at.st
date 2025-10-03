-- ユーザーテーブルにレベル関連のカラムを追加します。
-- level: ユーザーの現在のレベル。デフォルトは1。
-- last_level_up_at: 最後にレベルアップした日時。クールダウン（23時間）の判定に使用。
-- last_level_up_ip: 最後にレベルアップした際のIPアドレス。重複レベルアップ防止に使用。
-- last_level_up_fingerprint: 最後にレベルアップした際のFingerprintJS。複垢でのレベルアップ防止に使用。
ALTER TABLE users
ADD COLUMN level INTEGER NOT NULL DEFAULT 1,
ADD COLUMN last_level_up_at TIMESTAMPTZ,
ADD COLUMN last_level_up_ip TEXT,
ADD COLUMN last_level_up_fingerprint TEXT;

-- 投稿(スレッド)テーブルに投稿者のuser_idを追加します。
-- これにより、スレッド作成者のレベルも表示できるようになります。
-- コメントには既にuser_idが存在するため、postsテーブルのみ変更します。
-- 既存の投稿にはuser_idがないため、NULLを許容します。
ALTER TABLE posts
ADD COLUMN user_id INTEGER REFERENCES users(id) ON DELETE SET NULL;

-- アプリケーション全体の設定を管理するためのsettingsテーブルを新設します。
-- まずは、管理者が見えるレベルの閾値を管理するために使用します。
-- key: 設定項目名 (例: 'level_display_threshold')
-- value: 設定値 (例: '10')
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- settingsテーブルに、レベル表示の閾値の初期値を挿入します。
-- この例では、Lv.10以上のユーザーのレベルを非表示にする設定です。
-- この値は後から管理画面などで変更できるようにします。
INSERT INTO settings (key, value) VALUES ('level_display_threshold', '10');

-- updated_atを自動更新するためのトリガー関数を作成します。
CREATE OR REPLACE FUNCTION trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- settingsテーブルにトリガーを適用します。
CREATE TRIGGER set_timestamp
BEFORE UPDATE ON settings
FOR EACH ROW
EXECUTE FUNCTION trigger_set_timestamp();
