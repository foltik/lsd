ALTER TABLE posts ADD content_rendered TEXT NOT NULL;
UPDATE posts SET content_rendered = content;