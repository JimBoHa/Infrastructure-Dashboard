-- Split node health latency into explicit ICMP ping and MQTT broker RTT metrics.
-- Keep legacy network_* columns for backward compatibility.

ALTER TABLE nodes
  ADD COLUMN IF NOT EXISTS ping_ms double precision,
  ADD COLUMN IF NOT EXISTS ping_p50_30m_ms double precision,
  ADD COLUMN IF NOT EXISTS ping_jitter_ms double precision,
  ADD COLUMN IF NOT EXISTS mqtt_broker_rtt_ms double precision,
  ADD COLUMN IF NOT EXISTS mqtt_broker_rtt_jitter_ms double precision;
