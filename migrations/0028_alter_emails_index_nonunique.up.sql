DROP INDEX emails_post_list_address_unique;

CREATE INDEX emails_post_list_address
ON emails(address, post_id, list_id);
