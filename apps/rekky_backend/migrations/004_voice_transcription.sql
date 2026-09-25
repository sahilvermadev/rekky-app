CREATE TABLE voice_transcription_permissions (
  account_id uuid PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
  enabled boolean NOT NULL DEFAULT false,
  generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
  provider_id text NOT NULL DEFAULT 'openai' CHECK (provider_id = 'openai'),
  disclosure_version integer,
  updated_at timestamptz NOT NULL DEFAULT now(),
  CHECK (NOT enabled OR disclosure_version = 1)
);

CREATE TABLE voice_transcription_jobs (
  account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  draft_id text NOT NULL CHECK (char_length(draft_id) BETWEEN 16 AND 64),
  audio_sha256 text NOT NULL CHECK (char_length(audio_sha256) = 64),
  permission_generation bigint NOT NULL,
  attempt_id uuid NOT NULL,
  status text NOT NULL CHECK (status IN ('processing', 'failed', 'cancelled', 'completed')),
  lease_until timestamptz NOT NULL,
  capture_id uuid UNIQUE REFERENCES captures(id) ON DELETE CASCADE,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (account_id, draft_id)
);
CREATE INDEX voice_transcription_jobs_pending_idx
  ON voice_transcription_jobs(account_id, status, lease_until);
