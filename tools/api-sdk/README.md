# API SDK Generator

This toolchain keeps the core OpenAPI contract and generated SDKs in sync across the repo.

## What it does

- Exports the Rust core-server OpenAPI schema to `apps/core-server-rs/openapi/farm-dashboard.json`.
- Merges contract-only schemas (MQTT telemetry + heartbeat payloads).
- Generates SDKs for:
  - Dashboard web (TypeScript fetch client).
  - Node agent (Python pydantic models).

## Prerequisites

- Python 3.11+
- Node 20+
- Java 17+ (for `openapi-generator-cli`)
- Rust toolchain (for Rust OpenAPI export)

## Install generator CLI

```bash
npm --prefix tools/api-sdk install
```

## Regenerate SDKs

```bash
python tools/api-sdk/generate.py
```

To regenerate a subset:

```bash
python tools/api-sdk/generate.py --targets ts
```

## Output locations

- OpenAPI contract: `apps/core-server-rs/openapi/farm-dashboard.json`
- TS client: `apps/dashboard-web/src/lib/api-client/`
- Python models: `apps/node-agent/app/generated_api/`
