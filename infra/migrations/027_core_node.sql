-- Create a stable "Core" node representing the controller itself.
-- This allows attaching system/global sensors (forecasts, controller-wide metrics)
-- without inventing per-node hardware identities.
--
-- Safe, idempotent (no resets).

INSERT INTO nodes (id, name, status, last_seen, config, created_at)
VALUES (
  '00000000-0000-0000-0000-000000000001',
  'Core',
  'online',
  NOW(),
  jsonb_build_object('kind', 'core', 'system', true),
  NOW()
)
ON CONFLICT (id) DO NOTHING;

