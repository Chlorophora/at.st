-- Add up migration script here
-- Create a new ENUM type for user roles
CREATE TYPE user_role AS ENUM ('user', 'moderator', 'admin');

-- Add the role column to the users table with a default value of 'user'
ALTER TABLE users ADD COLUMN role user_role NOT NULL DEFAULT 'user';