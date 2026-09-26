-- Store only call reservations, never Google address content or queries.
CREATE TABLE place_lookup_attempts (
    id uuid PRIMARY KEY,
    owner_id uuid REFERENCES accounts(id) ON DELETE SET NULL,
    item_id uuid REFERENCES knowledge_items(id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX place_lookup_owner_time ON place_lookup_attempts(owner_id, created_at);
CREATE INDEX place_lookup_time ON place_lookup_attempts(created_at);
