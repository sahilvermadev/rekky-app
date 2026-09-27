-- Ask audio lives only in request memory. Short-lived text supports uncertain retries.
CREATE TABLE ask_dictations (
 id uuid PRIMARY KEY,
 owner_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
 audio_hash text NOT NULL,
 status text NOT NULL CHECK(status IN ('running','completed','failed','cancelled')),
 permission_generation bigint NOT NULL,
 transcript text,
 charged boolean NOT NULL DEFAULT true,
 created_at timestamptz NOT NULL DEFAULT now(),
 expires_at timestamptz NOT NULL DEFAULT now() + interval '10 minutes'
);
CREATE INDEX ask_dictations_owner_created_idx ON ask_dictations(owner_id,created_at);
