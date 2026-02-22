# RUN-20260206 Installed-Controller Smoke Main-Integrity Cleanup

- **Date (UTC):** 2026-02-06
- **Scope:** DT-73, ARCH-2, ARCH-3, ARCH-4 smoke validation on a host with an installed controller stack already running.
- **Host type:** Installed controller host (smoke only; **not** a Tier‑A rebuild/refresh runbook).

## Notes

- This run does **not** include a controller rebuild/refresh/upgrade workflow; it only captures local smoke checks plus `e2e-installed-health-smoke` against the already-running installed stack.

## Commands

```bash
make ci-integrity-guardrail
make ci-farmctl-smoke
make ci-core-smoke
make ci-web-smoke-build
make e2e-installed-health-smoke
curl -fsS http://127.0.0.1:8000/healthz
```

## Result

- `make ci-integrity-guardrail`: **PASS**
- `make ci-farmctl-smoke`: **PASS**
- `make ci-core-smoke`: **PASS**
- `make ci-web-smoke-build`: **PASS**
- `make e2e-installed-health-smoke`: **PASS**
  - Report: `reports/e2e-installed-health-smoke/2026-02-06T23-29-28Z/`
- `curl -fsS http://127.0.0.1:8000/healthz`: `{"status":"ok"}`

## Viewed Screenshot Evidence

- `manual_screenshots_web/20260206_015437/trends_related_sensors_large_scan.png`
