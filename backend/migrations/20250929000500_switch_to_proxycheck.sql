-- `level_up_attempts`テーブルのカラムを変更
ALTER TABLE level_up_attempts
RENAME COLUMN spur_monocle_json TO proxycheck_json;

-- 古いspur_monocle_idカラムを削除
ALTER TABLE level_up_attempts
DROP COLUMN spur_monocle_id;