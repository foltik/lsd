CREATE INDEX emails_post_list_user_sent
ON emails(post_id, list_id, user_id, sent_at);
