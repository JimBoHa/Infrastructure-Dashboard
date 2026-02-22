# Telemetry Sidecar (Rust)

High-throughput MQTT -> Timescale/PostgreSQL sidecar used to offload metric ingestion from the FastAPI stack. It subscribes to `iot/{nodeId}/{sensorId}/telemetry` and `iot/{nodeId}/status`, applies rolling averages + change-of-value dedupe, updates node/sensor status + offline alarms, batches inserts via `sqlx` query builders, and exposes a small gRPC API over a UNIX domain socket so Python can push batches or query ingest health.

## Running locally

```bash
cd apps/telemetry-sidecar
cargo run
```

Key environment variables (defaults shown):

- `SIDECAR_DATABASE_URL` (required): Postgres/Timescale connection string.
- `SIDECAR_DB_POOL_SIZE` (default `10`): max connections in the pool.
- `SIDECAR_MQTT_HOST` / `SIDECAR_MQTT_PORT` (default `127.0.0.1:1883`).
- `SIDECAR_MQTT_USERNAME` / `SIDECAR_MQTT_PASSWORD` (optional).
- `SIDECAR_MQTT_TOPIC_PREFIX` (default `iot`).
- `SIDECAR_MQTT_CLIENT_ID` (default `telemetry-sidecar-<pid>`).
- `SIDECAR_BATCH_SIZE` (default `500`) and `SIDECAR_FLUSH_INTERVAL_MS` (default `750`).
- `SIDECAR_MAX_QUEUE` (default `batch_size * 10`).
- `SIDECAR_GRPC_SOCKET` (default `/tmp/telemetry_ingest.sock`).
- `SIDECAR_ENABLE_MQTT` (default `true`): disable if FastAPI is forwarding batches via gRPC to avoid double ingest.
- `SIDECAR_OFFLINE_THRESHOLD_SECONDS` (default `5`): offline threshold used for sensor/node status.
- `SIDECAR_STATUS_POLL_INTERVAL_MS` (default `1000`): offline check cadence.
- `SIDECAR_PREDICTIVE_FEED_URL` (optional): HTTP endpoint for predictive ingest (ex: `http://127.0.0.1:8000/api/predictive/ingest`).
- `SIDECAR_PREDICTIVE_FEED_TOKEN` (optional): shared token for `X-Predictive-Ingest-Token`.
- `SIDECAR_PREDICTIVE_FEED_BATCH_SIZE` (default `200`) and `SIDECAR_PREDICTIVE_FEED_FLUSH_MS` (default `500`).
- `SIDECAR_PREDICTIVE_FEED_QUEUE` (default `batch_size * 4`).
- `OTEL_EXPORTER_OTLP_ENDPOINT` (optional): enable OpenTelemetry/OTLP tracing output.

## gRPC surface (UNIX socket)

The proto definition lives in [`../../proto/ingest.proto`](../../proto/ingest.proto). The server listens on the path from `SIDECAR_GRPC_SOCKET` and exposes:

- `GetHealth`: queue depth, last flush timestamp/batch size, average flush duration, MQTT connectivity, and last error.
- `PushMetrics`: enqueues metrics (optionally `force_flush=true`).
- `Flush`: immediate flush + health snapshot.

Python connects with `grpcio` using the same socket (`unix:///tmp/telemetry_ingest.sock`).

## Backpressure + batching

- Bounded channel (`SIDECAR_MAX_QUEUE`) provides backpressure; producers await when full.
- Flush triggers on interval or when `SIDECAR_BATCH_SIZE` is reached.
- `sqlx::QueryBuilder` performs vectorized multi-row inserts with `ON CONFLICT DO NOTHING` to avoid PK churn.

## Observability

Set `OTEL_EXPORTER_OTLP_ENDPOINT` to emit tracing spans via OTLP (tonic gRPC). Structured logs are emitted via `tracing` with env-driven filters (`RUST_LOG`).
