-- Add up migration script here

-- Add columns to store the daily display ID
ALTER TABLE posts ADD COLUMN display_user_id VARCHAR(255);
ALTER TABLE comments ADD COLUMN display_user_id VARCHAR(255);

-- Add columns to store permanent hashes for banning
ALTER TABLE posts ADD COLUMN permanent_user_hash VARCHAR(255);
ALTER TABLE comments ADD COLUMN permanent_user_hash VARCHAR(255);
ALTER TABLE posts ADD COLUMN permanent_ip_hash VARCHAR(255);
ALTER TABLE comments ADD COLUMN permanent_ip_hash VARCHAR(255);
ALTER TABLE posts ADD COLUMN permanent_device_hash VARCHAR(255);
ALTER TABLE comments ADD COLUMN permanent_device_hash VARCHAR(255);