DROP INDEX emails_post_list_address;

CREATE UNIQUE INDEX emails_post_list_address_unique
ON emails(address, post_id, list_id);
