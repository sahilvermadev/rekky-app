-- Re-enrich saved locations with explicit city context whose neighbourhood
-- is absent from the local gazetteer. This changes filtering metadata only.
UPDATE geographic_catalog SET revision=revision+1 WHERE singleton;
