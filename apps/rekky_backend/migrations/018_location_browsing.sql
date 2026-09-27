-- Rebuild only derived location metadata with city-first browsing scopes.
UPDATE geographic_catalog SET revision=revision+1 WHERE singleton AND revision>0;
