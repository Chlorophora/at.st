-- Add migration script here
-- 新しいENUM型を定義
CREATE TYPE rate_limit_action_type AS ENUM ('CreateBoard', 'CreatePost', 'CreateComment');

-- rate_limit_rulesテーブルに新しいカラムを追加
ALTER TABLE rate_limit_rules
ADD COLUMN action_type rate_limit_action_type;

-- (任意) 既存のルールがある場合、デフォルトのアクションタイプを設定します
-- 例: UPDATE rate_limit_rules SET action_type = 'CreateComment' WHERE action_type IS NULL;

-- カラムにNOT NULL制約を追加
ALTER TABLE rate_limit_rules
ALTER COLUMN action_type SET NOT NULL;