-- Persistent UI ordering for nodes + sensors (dashboard display order).
-- This is used to keep node/sensor ordering consistent across dashboard surfaces.

ALTER TABLE nodes
  ADD COLUMN IF NOT EXISTS ui_order integer;

ALTER TABLE sensors
  ADD COLUMN IF NOT EXISTS ui_order integer;

-- Backfill existing rows to preserve the current "created_at" ordering as the default.
WITH ranked AS (
  SELECT id, row_number() OVER (ORDER BY created_at ASC, id ASC) AS rn
  FROM nodes
)
UPDATE nodes
SET ui_order = ranked.rn
FROM ranked
WHERE nodes.id = ranked.id
  AND nodes.ui_order IS NULL;

WITH ranked AS (
  SELECT sensor_id,
         row_number() OVER (PARTITION BY node_id ORDER BY created_at ASC, sensor_id ASC) AS rn
  FROM sensors
  WHERE deleted_at IS NULL
)
UPDATE sensors
SET ui_order = ranked.rn
FROM ranked
WHERE sensors.sensor_id = ranked.sensor_id
  AND sensors.ui_order IS NULL;

CREATE INDEX IF NOT EXISTS idx_nodes_ui_order ON nodes (ui_order);
CREATE INDEX IF NOT EXISTS idx_sensors_node_ui_order ON sensors (node_id, ui_order);

