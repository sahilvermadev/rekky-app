-- Private derivative, deleted with its raw source. No separate text history.
ALTER TABLE source_texts ADD COLUMN readable_content text;
ALTER TABLE source_texts ADD COLUMN readable_source_revision integer;
ALTER TABLE source_texts ADD COLUMN readable_version integer;
ALTER TABLE source_texts ADD CONSTRAINT readable_source_complete CHECK (
  (readable_content IS NULL AND readable_source_revision IS NULL AND readable_version IS NULL)
  OR (readable_content IS NOT NULL AND readable_source_revision IS NOT NULL AND readable_version IS NOT NULL AND readable_version = 1)
);
