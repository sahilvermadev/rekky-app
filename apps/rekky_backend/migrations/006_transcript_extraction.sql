CREATE TABLE transcript_extraction_permissions (
  account_id uuid PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
  enabled boolean NOT NULL DEFAULT false,
  generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
  disclosure_version integer,
  updated_at timestamptz NOT NULL DEFAULT now(),
  CHECK (NOT enabled OR disclosure_version = 1)
);

CREATE TABLE transcript_extraction_jobs (
  capture_id uuid PRIMARY KEY REFERENCES captures(id) ON DELETE CASCADE,
  account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  source_revision integer NOT NULL,
  permission_generation bigint NOT NULL,
  attempt_id uuid NOT NULL,
  status text NOT NULL CHECK (status IN ('processing','failed','cancelled','completed')),
  attempts integer NOT NULL DEFAULT 1 CHECK (attempts BETWEEN 1 AND 3),
  lease_until timestamptz NOT NULL,
  partial boolean,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX transcript_extraction_jobs_account_created_idx
  ON transcript_extraction_jobs(account_id, created_at DESC);
