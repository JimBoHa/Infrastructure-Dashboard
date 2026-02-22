-- The sensors.config latest_value/latest_ts trigger was a hot-path production workaround.
-- It adds an UPDATE to sensors on every metrics insert and stores latest_ts as text.
-- We now compute latest values at read time (JOIN to metrics) and keep raw metrics indefinitely.

DROP TRIGGER IF EXISTS trg_metrics_sensor_latest_value ON metrics;
DROP FUNCTION IF EXISTS sensor_latest_value_from_metrics();

-- Support efficient "latest per sensor" reads (DISTINCT ON / ORDER BY ts DESC) without relying
-- on a full-table scan.
CREATE INDEX IF NOT EXISTS idx_metrics_sensor_ts_desc ON metrics (sensor_id, ts DESC);

