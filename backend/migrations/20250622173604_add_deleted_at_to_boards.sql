-- Add migration script here
ALTER TABLE boards
ADD COLUMN deleted_at TIMESTAMPTZ;
