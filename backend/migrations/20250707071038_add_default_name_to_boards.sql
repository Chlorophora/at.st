-- Add migration script here
ALTER TABLE boards
ADD COLUMN default_name VARCHAR(255) NOT NULL DEFAULT '鶏民';
