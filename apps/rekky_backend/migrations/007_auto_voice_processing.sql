ALTER TABLE captures ADD COLUMN auto_processing boolean NOT NULL DEFAULT false;
ALTER TABLE captures ADD COLUMN auto_permission_generation bigint;
ALTER TABLE captures ADD COLUMN auto_next_attempt_at timestamptz NOT NULL DEFAULT now();
CREATE INDEX captures_auto_processing_idx
  ON captures(created_at)
  WHERE kind='voice' AND auto_processing AND status='transcript_ready';

-- Bounded temporary upload storage. Clear audio in the same transaction that
-- commits its accepted transcript; receipt metadata remains for lost responses.
CREATE TABLE voice_uploads (
  account_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  draft_id text NOT NULL CHECK (char_length(draft_id) BETWEEN 16 AND 64),
  audio bytea CHECK (octet_length(audio) BETWEEN 128 AND 5242880),
  audio_sha256 text NOT NULL,
  captured_ms bigint NOT NULL,
  voice_generation bigint NOT NULL,
  extraction_generation bigint NOT NULL,
  status text NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','processing','transcribed','failed','cancelled','expired')),
  attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 3),
  next_attempt_at timestamptz NOT NULL DEFAULT now(),
  capture_id uuid REFERENCES captures(id) ON DELETE CASCADE,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(account_id,draft_id)
);
CREATE INDEX voice_uploads_pending_idx ON voice_uploads(next_attempt_at)
  WHERE status IN ('queued','processing');
