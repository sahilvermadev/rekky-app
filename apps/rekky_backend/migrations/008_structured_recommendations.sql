ALTER TABLE knowledge_items ADD COLUMN recommendation jsonb;
ALTER TABLE transcript_extraction_jobs ADD COLUMN pipeline_version integer NOT NULL DEFAULT 1;
ALTER TABLE transcript_extraction_jobs ALTER COLUMN pipeline_version SET DEFAULT 2;

-- Source evidence is intentionally separate from shareable presentation fields.
-- Explicit source deletion also deletes this derivative copy of source text.
CREATE TABLE item_source_support (
  item_id uuid PRIMARY KEY REFERENCES knowledge_items(id) ON DELETE CASCADE,
  source_id uuid NOT NULL REFERENCES source_texts(id) ON DELETE CASCADE,
  source_revision integer NOT NULL,
  pipeline_version integer NOT NULL,
  support jsonb NOT NULL
);

-- Explicit upgrade of a pre-v2 single-item capture, never an automatic backfill.
-- Keeps the original extraction budget/receipt intact and has its own bounded
-- attempts counted against the same daily paid-work budget.
CREATE TABLE recommendation_refinement_jobs (
  item_id uuid PRIMARY KEY REFERENCES knowledge_items(id) ON DELETE CASCADE,
  account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  attempt_id uuid NOT NULL,
  status text NOT NULL CHECK(status IN ('processing','failed','completed','cancelled')),
  attempts integer NOT NULL DEFAULT 1 CHECK(attempts BETWEEN 1 AND 3),
  permission_generation bigint NOT NULL,
  source_revision integer NOT NULL,
  item_revision integer NOT NULL,
  lease_until timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX recommendation_refinement_account_idx ON recommendation_refinement_jobs(account_id,created_at);
