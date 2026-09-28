-- Ephemeral, account-bound query references; never copies of source transcripts.
CREATE TABLE ask_views (
 id uuid PRIMARY KEY,
 owner_id uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
 spec jsonb NOT NULL,
 fingerprint text NOT NULL,
 item_ids uuid[] NOT NULL,
 metadata jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 expires_at timestamptz NOT NULL DEFAULT now()+interval '15 minutes'
);
CREATE INDEX ask_views_owner_expiry ON ask_views(owner_id,expires_at);
ALTER TABLE ask_runs ADD COLUMN failure_code text;
-- Curated geographic identity data, not model prompt aliases. New Delhi is part
-- of the Delhi urban scope; this does not include all NCR or the NCT district.
CREATE TABLE ask_area_scopes (area_id text PRIMARY KEY, scope_id text NOT NULL);
INSERT INTO ask_area_scopes VALUES ('geonames:1261481','geonames:1273294');
