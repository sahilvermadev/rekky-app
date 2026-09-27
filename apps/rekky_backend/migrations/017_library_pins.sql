ALTER TABLE knowledge_items ADD COLUMN pinned boolean NOT NULL DEFAULT false;
ALTER TABLE knowledge_items ADD COLUMN pin_revision integer NOT NULL DEFAULT 1 CHECK (pin_revision > 0);
