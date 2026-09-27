CREATE TABLE category_registry_events (
  id bigserial PRIMARY KEY,
  registry_revision integer NOT NULL,
  action text NOT NULL CHECK(action IN ('create','alias','suspend')),
  concept_id text NOT NULL,
  receipt jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);
