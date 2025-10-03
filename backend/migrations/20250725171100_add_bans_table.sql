-- マイグレーションファイルに追加するSQL
ALTER TABLE bans ADD COLUMN encrypted_source_email BYTEA;
ALTER TABLE bans ADD COLUMN encrypted_source_ip BYTEA;
ALTER TABLE bans ADD COLUMN encrypted_source_device_info BYTEA;