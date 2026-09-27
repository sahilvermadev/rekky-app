-- Short-lived answer references; usage reservations survive expiry and crashes.
CREATE TABLE ask_runs (
 id uuid PRIMARY KEY,
 owner_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
 request_hash text NOT NULL,
 status text NOT NULL CHECK(status IN ('running','completed','failed','cancelled')),
 permission_generation bigint NOT NULL,
 result jsonb,
 decisions integer NOT NULL DEFAULT 0,
 input_tokens bigint NOT NULL DEFAULT 0,
 output_tokens bigint NOT NULL DEFAULT 0,
 reserved_microusd bigint NOT NULL DEFAULT 20000,
 created_at timestamptz NOT NULL DEFAULT now(),
 expires_at timestamptz NOT NULL DEFAULT now() + interval '15 minutes'
);
CREATE INDEX ask_runs_owner_created_idx ON ask_runs(owner_id,created_at);
