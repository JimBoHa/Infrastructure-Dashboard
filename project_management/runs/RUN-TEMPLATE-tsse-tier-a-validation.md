# RUN-TEMPLATE: TSSE Tier‑A validation (installed controller; no DB reset)

Date:

Operator:

Goal: Record Tier‑A evidence for TSSE features on the **installed controller** without DB/settings reset. This template is intended to satisfy `TSSE-1` Tier‑A evidence requirements.

References:
- `docs/runbooks/controller-rebuild-refresh-tier-a.md` (Tier‑A rebuild/refresh SOP)
- `project_management/TASKS.md` (TSSE-1 evidence requirements)

---

## Preconditions (hard gates)

- [ ] **No DB/settings reset performed** (Tier‑A rule).
- [ ] Installed setup daemon is reachable:
  - [ ] `curl -fsS http://127.0.0.1:8800/healthz`
- [ ] Installed core server is reachable:
  - [ ] `curl -fsS http://127.0.0.1:8000/healthz`
- [ ] Repo worktree is clean (Tier‑A hard gate for bundle build; `reports/**` may be dirty):
  - [ ] `git status --porcelain=v1 -b`

---

## Build + refresh (Tier‑A)

> Follow the SOP in `docs/runbooks/controller-rebuild-refresh-tier-a.md`. Record exact commands + final PASS/FAIL only (no streaming logs).

### Version
- Installed version before:
  - Command: `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - Result:
- New version (bundle build):
  - Chosen:

### Commands run (append-only)

- `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version <VER> --output <DMG> --native-deps /usr/local/farm-dashboard/native |& tee <LOG>`
- `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"<DMG>"}'`
- `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
- `make e2e-installed-health-smoke`

### Refresh verification
- [ ] `curl -fsS http://127.0.0.1:8000/healthz` (PASS)
- [ ] Installed version after upgrade recorded above (PASS)

---

## TSSE-specific evidence (Tier‑A)

### 1) Bench report (TSE-0019)

> Bench artifacts must land under `reports/` (Tier‑A allowlist). The report should explicitly show p50/p95 vs targets and PASS/FAIL.

- Bench command (example):
  - `cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_bench -- --base-url=http://127.0.0.1:8000 --auth-token-file=<TOKEN> --focus-sensor-id=<SENSOR_ID> --runs=7 --interval-seconds=60 --report reports/tsse-bench-YYYYMMDD_HHMM.md`
- Report path (must exist):
  - `reports/tsse-bench-YYYYMMDD_HHMM.md`
- Result:
  - [ ] PASS
  - [ ] FAIL (attach short failure summary + next action)

### 2) TSSE UI screenshots (must be captured, viewed, and hard-gated)

> Tier‑A UI evidence requires at least one screenshot that was opened and visually reviewed. Store under `manual_screenshots_web/` and reference the path(s) here.

- Screenshot command (example):
  - `node apps/dashboard-web/scripts/web-screenshots.mjs --no-web --no-core --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=<TOKEN>`
- Screenshot evidence (paths; confirm each was opened/viewed):
  - [ ] `manual_screenshots_web/<TIMESTAMP>/<FILE>.png` (VIEWED)
  - [ ] `manual_screenshots_web/<TIMESTAMP>/<FILE>.png` (VIEWED)

### 2b) Tier-A screenshot review hard gate (required)

```md
## Tier A Screenshot Review (Hard Gate)

- [x] REVIEWED: `manual_screenshots_web/<TIMESTAMP>/<FILE>.png`
- [x] REVIEWED: `manual_screenshots_web/<TIMESTAMP>/<FILE>.png`

### Visual checks (required)
- [x] PASS: `<FILE>.png` <what was checked and why it passes>
- [x] PASS: `<FILE>.png` <what was checked and why it passes>
- [x] PASS: `<FILE>.png` <what was checked and why it passes>

### Findings
- <Issue/finding or explicit "No blocking issues found" note>

### Reviewer declaration
I viewed each screenshot listed above.
```

- Hard-gate command:
  - `make tier-a-screenshot-gate RUN_LOG=project_management/runs/<RUN_FILE>.md`
- Result:
  - [ ] PASS
  - [ ] FAIL (run is not Tier-A complete)

Recommended TSSE capture targets (pick at least one):
- Trends → Related Sensors (job submit + result visible)
- Trends → Relationships / correlation matrix (job submit + matrix rendered)
- Trends → Co-occurrence (job submit + buckets/results visible)
- Trends → Matrix Profile (job submit + result visible)

### 3) API sanity (auth + caps)

- [ ] Confirm analysis endpoints are auth-gated (no token / missing caps should 403):
  - [ ] `POST /api/analysis/jobs` requires `analysis.run`
  - [ ] `POST /api/analysis/preview` requires `analysis.view`

---

## Evidence summary (copy/paste into TSSE-1 run log)

- Installed version before:
- Installed version after:
- Installed health smoke: PASS/FAIL
- Bench report path:
- Screenshot path(s) (VIEWED):
- Notes / anomalies:
