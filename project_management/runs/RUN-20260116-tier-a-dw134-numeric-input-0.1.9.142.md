# RUN-20260116-tier-a-dw134-numeric-input-0.1.9.142

- **Date (UTC):** 2026-01-16
- **Tier:** A (installed controller; no DB/settings reset)
- **Controller version:** 0.1.9.142
- **Purpose:** Validate dashboard numeric input UX after DW-132 (decimals, intermediate range typing, negative sign entry where allowed).

## Preconditions
- Installed controller stack running.
- Dashboard reachable at `http://127.0.0.1:8000`.

## Confirm installed version
- `curl -fsS http://127.0.0.1:8800/api/status | jq -r '.logs[0].stdout' | jq '{current_version, previous_version}'`

## Validation Steps (UI)
- Opened **Sensors & Outputs** → **Add sensor** → **ADC** and verified numeric fields accept decimals (e.g., `0.25`) and negatives for offset (e.g., `-1`).
- Opened **Nodes** → a node’s **Local display** section and verified a range-restricted integer field accepts intermediate out-of-range typing (e.g., typing `12` works even though `1` is < min).

## Evidence
- **Screenshots (captured + viewed):**
  - `manual_screenshots_web/tier_a_0.1.9.142_dw134_numeric_input_2026-01-16_033658858Z/01_add_sensor_adc_numeric.png`
  - `manual_screenshots_web/tier_a_0.1.9.142_dw134_numeric_input_2026-01-16_033658858Z/02_range_restricted_jitter_window.png`

## Result
- **PASS**

