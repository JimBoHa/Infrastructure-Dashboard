-- Add node health snapshot fields for fleet ops telemetry (Tier A).
-- Safe additive migration (no resets).

ALTER TABLE nodes
  ADD COLUMN IF NOT EXISTS cpu_percent_per_core jsonb,
  ADD COLUMN IF NOT EXISTS memory_percent real,
  ADD COLUMN IF NOT EXISTS memory_used_bytes bigint,
  ADD COLUMN IF NOT EXISTS network_latency_ms real,
  ADD COLUMN IF NOT EXISTS network_jitter_ms real,
  ADD COLUMN IF NOT EXISTS uptime_percent_24h real;

