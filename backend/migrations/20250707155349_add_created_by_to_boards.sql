-- Add migration script here
ALTER TABLE boards
ADD COLUMN created_by INTEGER REFERENCES users(id) ON DELETE SET NULL;

-- 既存の板がある場合、それらを特定のユーザー（例: ID=1の管理者）に割り当てることができます。
-- これは任意ですが、既存の板にも所有者を設定したい場合に実行してください。
-- UPDATE boards SET created_by = 1 WHERE created_by IS NULL;