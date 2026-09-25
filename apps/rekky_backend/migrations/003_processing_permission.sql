CREATE TABLE processing_permissions (
  account_id uuid PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
  enabled boolean NOT NULL DEFAULT false,
  generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
  purpose text,
  provider_ids text[] NOT NULL DEFAULT '{}',
  disclosure_version integer,
  updated_at timestamptz NOT NULL DEFAULT now(),
  CHECK (NOT enabled OR (purpose IS NOT NULL AND cardinality(provider_ids) > 0 AND disclosure_version IS NOT NULL))
);
