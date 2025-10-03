-- Add migration script here
ALTER TABLE comments ADD COLUMN author_name VARCHAR(255);