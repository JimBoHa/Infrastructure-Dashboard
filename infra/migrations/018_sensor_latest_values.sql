-- Deprecated: this migration originally introduced a hot-path trigger that updated
-- `sensors.config.latest_value/latest_ts` on every metrics insert (and stored latest_ts as text).
-- The Rust core-server now provides latest values by joining `metrics` at read time, keeping raw
-- telemetry indefinitely without incurring per-sample UPDATE overhead.

DROP TRIGGER IF EXISTS trg_metrics_sensor_latest_value ON metrics;
DROP FUNCTION IF EXISTS sensor_latest_value_from_metrics();
