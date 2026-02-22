# Observability (Logs + Traces)

This repo standardizes structured JSON logging and OpenTelemetry traces across:

- `apps/core-server-rs`
- `apps/telemetry-sidecar`
- `apps/node-agent`

## Structured logging format

Each log line is JSON with consistent fields:

- `timestamp` (UTC ISO8601)
- `level` (`INFO`, `WARN`, etc.)
- `message`
- `logger`
- `service` (`core-server`, `node-agent`)
- `request_id` (propagated via HTTP or MQTT payloads when available)
- `trace_id` / `span_id` (OpenTelemetry correlation)

## Local collector (Tempo)

Local configs live in `infra/otel-collector-config.yaml` and `infra/tempo.yaml`. Use the installer/launchd-managed Grafana/Tempo if available, or run local binaries with these configs and then:

1. Visit Grafana at `http://localhost:3000` (admin / admin)
2. Open the **Observability Quickstart** dashboard or use **Explore** with the Tempo datasource.

## Enabling tracing

Core server:

```bash
export CORE_OTEL_ENABLED=true
export CORE_OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317
```

Node agent:

```bash
export NODE_OTEL_ENABLED=true
export NODE_OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317
```

Optional:

```bash
export CORE_OTEL_EXPORTER_OTLP_HEADERS="authorization=Bearer <token>"
export NODE_OTEL_EXPORTER_OTLP_HEADERS="authorization=Bearer <token>"
```

## Forwarding to SaaS backends

Two recommended approaches:

1. **Collector fan-out**: add a second exporter in `infra/otel-collector-config.yaml` and route traces to Tempo + SaaS.
2. **Direct exporter**: point `CORE_OTEL_EXPORTER_OTLP_ENDPOINT` / `NODE_OTEL_EXPORTER_OTLP_ENDPOINT` directly at your vendor OTLP endpoint.

When routing through the collector, set vendor-specific headers in the collector config. When routing directly, set
`*_OTEL_EXPORTER_OTLP_HEADERS` with a comma-separated `key=value` list.

## Request ID propagation

- Incoming HTTP requests accept `X-Request-ID` and echo it in responses.
- Core server publishes MQTT command payloads with the same `request_id`.
- Node agent echoes `request_id` in output state acknowledgements.
- Telemetry payloads include a generated `request_id` to assist correlation.
