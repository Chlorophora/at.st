-- Add migration script here
ALTER TABLE posts ADD COLUMN author_name VARCHAR(255);
