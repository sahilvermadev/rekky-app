ALTER TABLE transcript_extraction_jobs ADD COLUMN repair_attempted boolean NOT NULL DEFAULT false;

-- Private diagnostic data is source-linked and never returned by item/search APIs.
CREATE TABLE understanding_attempts (
  attempt_id uuid NOT NULL,
  stage text NOT NULL CHECK(stage IN ('understand','repair','refine')),
  source_id uuid NOT NULL REFERENCES source_texts(id) ON DELETE CASCADE,
  created_at timestamptz NOT NULL DEFAULT now(),
  outcome text NOT NULL DEFAULT 'started',
  duration_ms bigint,
  metadata jsonb NOT NULL DEFAULT '{}',
  issues jsonb NOT NULL DEFAULT '[]',
  candidate jsonb,
  PRIMARY KEY(attempt_id,stage)
);
CREATE INDEX understanding_attempts_source_idx ON understanding_attempts(source_id);
