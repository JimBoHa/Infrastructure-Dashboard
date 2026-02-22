-- Renogy BT-2 settings: apply queue + guardrails (maintenance mode)

ALTER TABLE device_settings
  ADD COLUMN IF NOT EXISTS apply_requested boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS apply_requested_at timestamptz NULL,
  ADD COLUMN IF NOT EXISTS apply_requested_by uuid NULL REFERENCES users(id),
  ADD COLUMN IF NOT EXISTS last_apply_attempt_at timestamptz NULL,
  ADD COLUMN IF NOT EXISTS apply_attempts int NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS maintenance_mode boolean NOT NULL DEFAULT false;

