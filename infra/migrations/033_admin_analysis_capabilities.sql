-- Backfill TSSE analysis capabilities for existing admin users.
--
-- Older installs may have admin users created before `analysis.view`/`analysis.run`
-- were added to the default admin capability set. This migration is idempotent and
-- only appends missing elements to the JSONB capabilities array.

UPDATE users
SET capabilities = capabilities || '["analysis.view"]'::jsonb
WHERE lower(trim(role)) = 'admin'
  AND NOT (capabilities ? 'analysis.view');

UPDATE users
SET capabilities = capabilities || '["analysis.run"]'::jsonb
WHERE lower(trim(role)) = 'admin'
  AND NOT (capabilities ? 'analysis.run');

