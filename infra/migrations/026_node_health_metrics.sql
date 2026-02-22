-- Node health telemetry additions.
ALTER TABLE nodes ADD COLUMN IF NOT EXISTS memory_used_bytes bigint DEFAULT 0;
ALTER TABLE nodes ADD COLUMN IF NOT EXISTS network_latency_ms double precision;
ALTER TABLE nodes ADD COLUMN IF NOT EXISTS network_jitter_ms double precision;
ALTER TABLE nodes ADD COLUMN IF NOT EXISTS uptime_percent_24h real;
