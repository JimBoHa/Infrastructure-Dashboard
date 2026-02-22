-- Renogy BT-2 controller settings (desired state + audit history)

CREATE TABLE IF NOT EXISTS device_settings (
  node_id uuid NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
  device_type text NOT NULL,
  desired jsonb NOT NULL DEFAULT '{}'::jsonb,
  desired_updated_at timestamptz NOT NULL DEFAULT now(),
  desired_updated_by uuid NULL REFERENCES users(id),
  last_applied jsonb NULL,
  last_applied_at timestamptz NULL,
  last_applied_by uuid NULL REFERENCES users(id),
  last_apply_status text NULL,
  last_apply_result jsonb NULL,
  pending boolean NOT NULL DEFAULT false,
  PRIMARY KEY (node_id, device_type)
);

CREATE TABLE IF NOT EXISTS device_settings_events (
  id bigserial PRIMARY KEY,
  node_id uuid NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
  device_type text NOT NULL,
  event_type text NOT NULL,
  actor_user_id uuid NULL REFERENCES users(id),
  created_at timestamptz NOT NULL DEFAULT now(),
  desired jsonb NULL,
  current jsonb NULL,
  diff jsonb NULL,
  result jsonb NULL
);

CREATE INDEX IF NOT EXISTS device_settings_events_node_id_idx
  ON device_settings_events (node_id, created_at DESC);

