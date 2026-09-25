-- A source or item may only refer to a capture owned by the same account.
ALTER TABLE captures ADD CONSTRAINT captures_id_owner_unique UNIQUE (id, owner_id);
ALTER TABLE source_texts ADD CONSTRAINT source_capture_owner_fk FOREIGN KEY (capture_id, owner_id) REFERENCES captures (id, owner_id) ON DELETE CASCADE;
ALTER TABLE knowledge_items ADD CONSTRAINT item_capture_owner_fk FOREIGN KEY (capture_id, owner_id) REFERENCES captures (id, owner_id) ON DELETE CASCADE;
