-- Node-forwarder durability ACK state and loss-range receipts.
-- Used by telemetry-sidecar to publish iot/{node_id}/ack after DB commit so nodes can truncate spools safely.

CREATE TABLE IF NOT EXISTS node_forwarder_ack_state (
  node_mqtt_id TEXT PRIMARY KEY,
  stream_id UUID NOT NULL,
  acked_seq BIGINT NOT NULL DEFAULT 0,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS node_forwarder_loss_ranges (
  node_mqtt_id TEXT NOT NULL,
  stream_id UUID NOT NULL,
  start_seq BIGINT NOT NULL,
  end_seq BIGINT NOT NULL,
  dropped_at TIMESTAMPTZ,
  reason TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (node_mqtt_id, stream_id, start_seq, end_seq)
);

CREATE INDEX IF NOT EXISTS idx_node_forwarder_loss_ranges_node_stream
  ON node_forwarder_loss_ranges (node_mqtt_id, stream_id, end_seq);

