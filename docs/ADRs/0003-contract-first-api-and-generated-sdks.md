# 0003. Contract-first API and generated SDKs

* **Status:** Accepted
* **Date:** 2025-12-19

## Context
Each active surface (core server, dashboard web, node agent) has been maintaining its own models and type assumptions. This creates drift, silent decoding failures, and higher integration cost when API shapes change. We need a single, versioned contract and a repeatable way to propagate it across clients.

## Decision
Adopt a contract-first workflow anchored on a master OpenAPI specification exported from the core server. Use `tools/api-sdk/generate.py` to:

- Export the Rust core-server OpenAPI schema to `apps/core-server-rs/openapi/farm-dashboard.json`.
- Merge contract-only schemas (MQTT telemetry + heartbeat payloads) via `tools/api-sdk/openapi_extras.json`.
- Generate SDKs for:
  - Dashboard web (TypeScript fetch client in `apps/dashboard-web/src/lib/api-client/`).
  - Node agent (Python Pydantic models in `apps/node-agent/app/generated_api/`).

CI runs an `api_sdk` job to regenerate SDKs and fail if the repo is out of sync. Application code should import types from the generated SDKs rather than duplicating API shapes.

## Consequences
- Easier: consistent contracts across services, reliable client updates, and safer refactors.
- More difficult: API changes now require regenerating SDKs and updating client adapters.
- Risks: generator tooling adds build dependencies (Java + Node) and may require occasional template configuration.
