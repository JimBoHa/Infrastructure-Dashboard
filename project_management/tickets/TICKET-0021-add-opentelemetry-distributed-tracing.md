Add OpenTelemetry Distributed Tracing

  Priority: P2 (Observability)
  Status: To Do
  Estimated Effort: Medium (4-8 hours)

  Problem

  core-server-rs has no distributed tracing, while telemetry-sidecar has full OpenTelemetry support. This makes debugging cross-service issues difficult.

  Current State

  telemetry-sidecar (has tracing):
  opentelemetry = { version = "0.23", features = ["trace"] }
  opentelemetry-otlp = { version = "0.16", ... }
  opentelemetry_sdk = { version = "0.23", features = ["trace", "rt-tokio"] }
  tracing-opentelemetry = "0.24"

  core-server-rs (no tracing):
  tracing = "0.1"
  tracing-subscriber = { version = "0.3", features = ["env-filter"] }
  # No opentelemetry deps

  Solution

  Add OpenTelemetry to core-server-rs matching the sidecar's setup:

  # apps/core-server-rs/Cargo.toml
  opentelemetry = { version = "0.23", features = ["trace"] }
  opentelemetry-otlp = { version = "0.16", default-features = false, features = ["http-proto", "reqwest-client", "trace"] }
  opentelemetry_sdk = { version = "0.23", features = ["trace", "rt-tokio"] }
  tracing-opentelemetry = "0.24"

  Acceptance Criteria

  - Add OpenTelemetry dependencies to Cargo.toml
  - Initialize OTLP exporter in main.rs (controlled by CORE_OTEL_ENABLED env var)
  - Add #[tracing::instrument] to key route handlers
  - Verify traces appear in Grafana Tempo (or OTEL collector)
  - Add trace context propagation for MQTT publishes

  Environment Variables

  CORE_OTEL_ENABLED=true
  OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
  OTEL_SERVICE_NAME=core-server-rs

