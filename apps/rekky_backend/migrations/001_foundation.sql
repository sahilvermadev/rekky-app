CREATE TABLE accounts (
  id uuid PRIMARY KEY,
  disclosure_accepted_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE account_identities (
  provider text NOT NULL CHECK (provider IN ('google', 'apple')),
  subject text NOT NULL,
  account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  PRIMARY KEY (provider, subject)
);
CREATE TABLE sessions (
  token_hash text PRIMARY KEY,
  account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  expires_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX sessions_account_idx ON sessions(account_id);
CREATE TABLE captures (
  id uuid PRIMARY KEY,
  owner_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  kind text NOT NULL CHECK (kind IN ('typed', 'voice', 'shared_text')),
  status text NOT NULL CHECK (status IN ('queued', 'transcript_ready', 'processing', 'partial', 'completed', 'failed', 'expired')),
  desired_visibility text NOT NULL CHECK (desired_visibility IN ('friends', 'private')),
  revision integer NOT NULL DEFAULT 1,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX captures_owner_idx ON captures(owner_id, created_at DESC);
CREATE TABLE source_texts (
  id uuid PRIMARY KEY,
  capture_id uuid NOT NULL UNIQUE REFERENCES captures(id) ON DELETE CASCADE,
  owner_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  kind text NOT NULL CHECK (kind IN ('typed', 'transcript', 'shared_text')),
  content text NOT NULL CHECK (char_length(content) BETWEEN 1 AND 20000),
  revision integer NOT NULL DEFAULT 1,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX source_texts_owner_idx ON source_texts(owner_id);
CREATE TABLE knowledge_items (
  id uuid PRIMARY KEY,
  capture_id uuid NOT NULL REFERENCES captures(id) ON DELETE CASCADE,
  owner_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  subject text NOT NULL CHECK (char_length(subject) BETWEEN 1 AND 120),
  body text NOT NULL CHECK (char_length(body) BETWEEN 1 AND 20000),
  visibility text NOT NULL CHECK (visibility IN ('friends', 'private')),
  revision integer NOT NULL DEFAULT 1,
  deleted_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX knowledge_items_owner_created_idx ON knowledge_items(owner_id, created_at DESC, id DESC) WHERE deleted_at IS NULL;
CREATE INDEX knowledge_items_search_idx ON knowledge_items USING gin (to_tsvector('simple', subject || ' ' || body)) WHERE deleted_at IS NULL;
CREATE TABLE idempotency_records (
  account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  operation text NOT NULL,
  request_key text NOT NULL,
  payload_hash text NOT NULL,
  response jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (account_id, operation, request_key)
);
