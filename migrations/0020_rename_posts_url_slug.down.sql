ALTER TABLE posts RENAME COLUMN slug TO url;
DROP INDEX IF EXISTS posts_slug_unique;
