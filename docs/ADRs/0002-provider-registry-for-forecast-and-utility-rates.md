# 0002. Provider registry for forecast and utility rates

* **Status:** Accepted
* **Date:** 2025-12-19

> **Implementation note (2026-01-16):** The production Rust controller does not implement the provider-registry/env-selector described in this ADR yet. Today it supports HTTP fixture ingestion via `CORE_FORECAST_API_BASE_URL` + `CORE_FORECAST_API_PATH` and `CORE_ANALYTICS_RATES__API_BASE_URL` + `CORE_ANALYTICS_RATES__API_PATH`.
> The `*_PROVIDER` selector and file-provider options mentioned below are deprecated in the Rust controller.

## Context
Forecast and utility rate ingestion are currently embedded inside their pollers, mixing transport (HTTP/file),
provider normalization, and persistence. This makes it hard to add new regions/providers without code changes,
and it is difficult to surface consistent "stale/missing" states for schedule guards and analytics consumers.
We need a registry-driven provider surface that allows operators to plug in file- or HTTP-backed data sources
without touching core business logic.

## Decision
Introduce a provider registry in `app/services/providers/` with explicit interfaces for forecast and rate
providers. Implement file- and HTTP-backed providers (plus mock/fixed defaults), and refactor the forecast
manager + utility rate feed to use the registry. Add staleness detection in API/status surfaces and schedule
guards, and ship contract tests against recorded fixtures so provider outputs stay consistent across regions.

Default behavior favors mock/file providers (no external secrets required), while HTTP providers can be enabled
by configuration and expect canonical JSON payloads. This keeps new providers/regions a configuration exercise
instead of a code change.

## Consequences
- Easier to add new providers by pointing at a file or HTTP endpoint that emits the canonical payload.
- Forecast/rate freshness is now explicit in API responses and schedule guard evaluation.
- Provider-specific normalization remains supported but is isolated behind the registry.
- Additional fixtures and provider tests must be maintained alongside new provider payloads.

## Onboarding steps (new region/provider, no code changes)
1. Decide on delivery: file or HTTP.
2. Emit the canonical payload:
   - Forecast: JSON with `samples` array of `{field,horizon_hours,value}` (optional `recorded_at`).
   - Rates: JSON with `provider`, `timezone`, `currency`, `periods` (plus `default_rate` if needed).
3. Configure env:
   - Forecast (Rust controller): `CORE_FORECAST_API_BASE_URL` + `CORE_FORECAST_API_PATH`
   - Rates (Rust controller): `CORE_ANALYTICS_RATES__API_BASE_URL` + `CORE_ANALYTICS_RATES__API_PATH`
4. Verify freshness: check `/api/forecast/status` and `/api/analytics/feeds/status` for `data_status`.
