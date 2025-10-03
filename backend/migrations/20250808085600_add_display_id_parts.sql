-- 1. postsテーブルに表示用IDの各部分を格納するカラムを追加
ALTER TABLE posts ADD COLUMN display_id_user VARCHAR(8);
ALTER TABLE posts ADD COLUMN display_id_ip VARCHAR(8);
ALTER TABLE posts ADD COLUMN display_id_device VARCHAR(8);

-- 2. commentsテーブルに表示用IDの各部分を格納するカラムを追加
ALTER TABLE comments ADD COLUMN display_id_user VARCHAR(8);
ALTER TABLE comments ADD COLUMN display_id_ip VARCHAR(8);
ALTER TABLE comments ADD COLUMN display_id_device VARCHAR(8);

-- 3. 既存のpostsデータのdisplay_user_idを分割して新しいカラムに格納 (バックフィル)
--    display_user_idが 'part1-part2-part3' の形式であることを前提としています。
--    NULLでないレコードのみを対象とします。
UPDATE posts
SET
    display_id_user = split_part(display_user_id, '-', 1),
    display_id_ip = split_part(display_user_id, '-', 2),
    display_id_device = split_part(display_user_id, '-', 3)
WHERE display_user_id IS NOT NULL AND display_user_id ~ '^[a-zA-Z0-9]+-[a-zA-Z0-9]+-[a-zA-Z0-9]+$';

-- 4. 既存のcommentsデータのdisplay_user_idを分割して新しいカラムに格納 (バックフィル)
UPDATE comments
SET
    display_id_user = split_part(display_user_id, '-', 1),
    display_id_ip = split_part(display_user_id, '-', 2),
    display_id_device = split_part(display_user_id, '-', 3)
WHERE display_user_id IS NOT NULL AND display_user_id ~ '^[a-zA-Z0-9]+-[a-zA-Z0-9]+-[a-zA-Z0-9]+$';

-- 5. パフォーマンス向上のため、新しい各カラムにインデックスを作成
--    これにより、IDの各部分での検索が非常に高速になります。
CREATE INDEX idx_posts_display_id_user ON posts(display_id_user);
CREATE INDEX idx_posts_display_id_ip ON posts(display_id_ip);
CREATE INDEX idx_posts_display_id_device ON posts(display_id_device);

CREATE INDEX idx_comments_display_id_user ON comments(display_id_user);
CREATE INDEX idx_comments_display_id_ip ON comments(display_id_ip);
CREATE INDEX idx_comments_display_id_device ON comments(display_id_device);

-- 6. (オプション) display_user_idカラムのインデックスは不要になる可能性があります。
--    DROP INDEX IF EXISTS idx_posts_display_user_id;
--    DROP INDEX IF EXISTS idx_comments_display_user_id;

