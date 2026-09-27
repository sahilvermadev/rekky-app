-- Global entries contain reviewed generic definitions only. Discovery pointers
-- remain owner/source-bound; no transcript, subject, contact or quote is copied.
CREATE TABLE category_registry_state (id boolean PRIMARY KEY DEFAULT true CHECK(id), revision integer NOT NULL DEFAULT 0);
INSERT INTO category_registry_state DEFAULT VALUES;
CREATE TABLE learned_categories (
  id text PRIMARY KEY,
  definition jsonb NOT NULL,
  active boolean NOT NULL DEFAULT true,
  revision integer NOT NULL DEFAULT 1,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE learned_category_aliases (
  normalized text PRIMARY KEY,
  phrase text NOT NULL,
  concept_id text NOT NULL,
  active boolean NOT NULL DEFAULT true
);
CREATE TABLE category_learning_jobs (
  key text PRIMARY KEY,
  status text NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','processing','accepted','provisional','failed')),
  attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 2),
  attempt_id uuid,
  lease_until timestamptz,
  retry_at timestamptz NOT NULL DEFAULT now(),
  concept_id text,
  reason text,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE category_discoveries (
  item_id uuid PRIMARY KEY REFERENCES knowledge_items(id) ON DELETE CASCADE,
  owner_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  source_id uuid NOT NULL REFERENCES source_texts(id) ON DELETE CASCADE,
  source_revision integer NOT NULL,
  permission_generation bigint NOT NULL,
  classification jsonb NOT NULL,
  job_key text NOT NULL REFERENCES category_learning_jobs(key),
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX category_discoveries_job_idx ON category_discoveries(job_key);
-- Each reservation permits at most one proposal and one independent review call.
CREATE TABLE category_learning_attempts (
  id uuid PRIMARY KEY,
  job_key text NOT NULL REFERENCES category_learning_jobs(key),
  owner_id uuid REFERENCES accounts(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX category_attempts_time_idx ON category_learning_attempts(created_at);
