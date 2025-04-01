UPDATE posts SET content = content_rendered;
ALTER TABLE posts DROP COLUMN content_rendered;