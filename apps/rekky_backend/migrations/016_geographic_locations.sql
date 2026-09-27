-- Openly licensed geographic data, separate from Google venue content.
CREATE TABLE geographic_areas (
    id text PRIMARY KEY,
    name text NOT NULL,
    label text NOT NULL,
    country text NOT NULL,
    feature text NOT NULL,
    population bigint NOT NULL,
    aliases text[] NOT NULL,
    ancestors text[] NOT NULL,
    hierarchy jsonb NOT NULL
);
CREATE INDEX geographic_area_aliases ON geographic_areas USING gin(aliases);
CREATE TABLE geographic_catalog (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    revision bigint NOT NULL DEFAULT 0,
    imported_at timestamptz
);
INSERT INTO geographic_catalog(singleton) VALUES(true);
CREATE TABLE item_location_index (
    item_id uuid NOT NULL REFERENCES knowledge_items(id) ON DELETE CASCADE,
    ordinal integer NOT NULL,
    role text NOT NULL,
    area_ids text[] NOT NULL,
    locations_snapshot jsonb NOT NULL,
    PRIMARY KEY(item_id,ordinal)
);
CREATE INDEX item_location_area_ids ON item_location_index USING gin(area_ids);
