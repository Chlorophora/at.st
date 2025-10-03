-- 20240101000000_add_auto_archive_to_boards.sql などのファイル名で作成
ALTER TABLE boards
ADD COLUMN auto_archive_enabled BOOLEAN NOT NULL DEFAULT TRUE;