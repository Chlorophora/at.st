-- Add migration script here
ALTER TABLE posts ADD COLUMN archived_at TIMESTAMPTZ;
-- 状態の不整合を避けるため、依存するインデックスも同じマイグレーションで作成する
CREATE INDEX IF NOT EXISTS idx_posts_board_active ON posts (board_id, deleted_at, archived_at, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_posts_active_created ON posts (deleted_at, archived_at, created_at DESC);