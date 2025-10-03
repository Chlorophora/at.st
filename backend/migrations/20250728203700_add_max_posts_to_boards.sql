-- migrations/{timestamp}_add_max_posts_to_boards.sql
ALTER TABLE boards
ADD COLUMN max_posts INTEGER NOT NULL DEFAULT 100;