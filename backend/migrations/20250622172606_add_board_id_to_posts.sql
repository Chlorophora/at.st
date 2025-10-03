-- Add migration script here
-- Add a board_id column to the posts table to link posts to boards.
ALTER TABLE posts
ADD COLUMN board_id INTEGER REFERENCES boards(id) ON DELETE RESTRICT;
