-- boardsテーブルにverification_attempt_idカラムを追加
ALTER TABLE boards
ADD COLUMN verification_attempt_id INTEGER REFERENCES level_up_attempts(id) ON DELETE SET NULL;

-- postsテーブルにverification_attempt_idカラムを追加
ALTER TABLE posts
ADD COLUMN verification_attempt_id INTEGER REFERENCES level_up_attempts(id) ON DELETE SET NULL;

-- commentsテーブルにverification_attempt_idカラムを追加
ALTER TABLE comments
ADD COLUMN verification_attempt_id INTEGER REFERENCES level_up_attempts(id) ON DELETE SET NULL;
