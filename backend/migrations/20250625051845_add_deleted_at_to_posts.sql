-- 20250625051845_add_deleted_at_to_posts.sql
ALTER TABLE posts ADD COLUMN deleted_at TIMESTAMPTZ;