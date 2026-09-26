-- Derived projections cannot drift when an item is edited/deleted. Raw evidence
-- remains in item_source_support and is not part of these search projections.
CREATE FUNCTION rekky_category_ids(doc jsonb) RETURNS text[]
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
  SELECT ARRAY(SELECT jsonb_array_elements_text(COALESCE(doc #> '{classification,search_ids}', '[]'::jsonb)))
$$;
ALTER TABLE knowledge_items ADD COLUMN category_ids text[]
  GENERATED ALWAYS AS (rekky_category_ids(recommendation)) STORED;
ALTER TABLE knowledge_items ADD COLUMN category_search text
  GENERATED ALWAYS AS (COALESCE(recommendation #>> '{classification,search_terms}', '')) STORED;
CREATE INDEX knowledge_items_categories_idx ON knowledge_items USING gin(category_ids)
  WHERE deleted_at IS NULL;
CREATE INDEX knowledge_items_classified_search_idx ON knowledge_items
  USING gin(to_tsvector('simple',subject || ' ' || body || ' ' || category_search))
  WHERE deleted_at IS NULL;
