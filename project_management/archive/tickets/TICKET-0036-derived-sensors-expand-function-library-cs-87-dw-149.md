# TICKET-0036: Derived sensors expand function library CS-87 DW-149

**Status:** Done (Tier A validated installed `0.1.9.162`; Tier B deferred to `DT-59` + `DW-98`)

## Description

Derived sensors let operators compute time-series sensors from other sensors using an expression language. The initial rollout supports basic operators and a small function set, but operators need richer functions (math + trig + conditional) and a UI that makes the full library discoverable.

This ticket expands:
- **Core-server derived expression language** with additional deterministic functions and strict domain validation (fail closed).
- **Dashboard Derived Sensor builder** with “insert” helpers and an expanded “More functions” library so operators can author expressions without guesswork.

Constraints (non-negotiable):
- No external/public API fallbacks: derived sensors operate only on controller-stored telemetry.
- Fail closed on invalid domains (e.g., `ln(x)` for `x <= 0`) and non-finite values.
- Do not obscure data: the UI should be explicit about function semantics (notably trig uses radians) and the sensor remains clearly labeled as `DERIVED`.

## Scope
* [ ] Add math/trig/conditional functions to the derived sensor expression evaluator with strict validation.
* [ ] Add unit tests covering the new functions.
* [ ] Update the dashboard Derived Sensor builder to list the full library and provide quick “insert” buttons.
* [ ] Make trig units explicit (radians) and include `deg2rad()` / `rad2deg()` guidance.
* [ ] Tier A validation on an installed controller: upgrade bundle, capture + view a screenshot, and write a run log.

## Acceptance Criteria
* [ ] Derived sensor expressions support: `floor`, `ceil`, `sqrt`, `pow`, `ln`, `log10`, `log`, `exp`, `sin`, `cos`, `tan`, `deg2rad`, `rad2deg`, `sign`, `if(cond,a,b)`.
* [ ] Invalid domains fail closed with clear errors (`ln(x)` requires `x > 0`, `sqrt(x)` requires `x >= 0`, etc.).
* [ ] Unit tests exist for the new functions and pass via `cargo test --manifest-path apps/core-server-rs/Cargo.toml`.
* [ ] Derived Sensor builder lists the extended function library and provides “insert” helpers for common functions.
* [ ] Trig UI copy explicitly states radians and points to `deg2rad()` / `rad2deg()`.
* [ ] `make ci-web-smoke` passes.
* [ ] Tier A validation is recorded with a run log under `project_management/runs/` and at least one captured + viewed screenshot under `manual_screenshots_web/`.

## Notes

**Primary file targets:**
- Backend: `apps/core-server-rs/src/services/derived_sensors.rs`
- UI: `apps/dashboard-web/src/features/sensors/components/DerivedSensorBuilder.tsx`

**Evidence:**
- Run: `project_management/runs/RUN-20260118-tier-a-cs87-dw149-derived-sensor-functions-0.1.9.162.md`
- Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.162_cs87_dw149_20260118_074353/derived_sensor_function_library.png`
