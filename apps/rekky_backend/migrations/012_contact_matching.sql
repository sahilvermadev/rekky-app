CREATE TABLE contact_matching_preferences (
    owner_id uuid PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    enabled boolean NOT NULL DEFAULT false,
    generation bigint NOT NULL DEFAULT 0,
    updated_at timestamptz NOT NULL DEFAULT now()
);
