-- Add level_at_creation to posts table
ALTER TABLE posts ADD COLUMN level_at_creation INTEGER;

-- Add level_at_creation to comments table
ALTER TABLE comments ADD COLUMN level_at_creation INTEGER;

