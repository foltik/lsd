ALTER TABLE posts RENAME COLUMN url TO slug;
CREATE UNIQUE INDEX IF NOT EXISTS posts_slug_unique ON posts(slug);
