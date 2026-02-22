# RUN-20260218 Tier A — CS-108/DW-258 battery SOC + power runway — Installed controller 0.1.9.274-battery-runway-fix

**Date:** 2026-02-18 (PST; 2026-02-18 UTC)

## Scope

- Validate battery SOC estimator + capacity + conservative power runway projection on the installed controller (Tier A; **no DB/settings reset**).
- Validate the Setup Center + Power tab UI readbacks on the installed controller with screenshot evidence.
- Create a warning alarm rule on the conservative runway sensor and confirm it evaluates without errors.

## Preconditions

- [x] No DB/settings reset performed (Tier‑A rule).
- [x] Installed setup daemon health OK: `curl -fsS http://127.0.0.1:8800/healthz`
- [x] Installed core server health OK: `curl -fsS http://127.0.0.1:8000/healthz`
- [x] Repo worktree clean for bundle build: `git status --porcelain=v1 -b`

## Build + Upgrade (Installed Controller; NO DB reset)

> Rebuild/refresh SOP: `docs/runbooks/controller-rebuild-refresh-tier-a.md`

- Installed version before:
  - `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - Result: `current_version=0.1.9.272-cs106-related-sensors`
- Built controller bundle DMG:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.274-battery-runway-fix.dmg`
- Build log:
  - `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.274-battery-runway-fix.log`
- Upgrade commands:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.274-battery-runway-fix.dmg"}'`
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Installed version after:
  - `curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'`
  - Result: `current_version=0.1.9.274-battery-runway-fix` (previous: `0.1.9.272-cs106-related-sensors`)

### Post-upgrade verification

- [x] `curl -fsS http://127.0.0.1:8000/healthz` (PASS)
- [x] `farmctl health --config /Users/Shared/FarmDashboard/setup/config.json` (PASS)
- [x] Installed smoke: `make e2e-installed-health-smoke` (PASS)

## CS-108 Evidence — Renogy + ADC-hat load runway (Tier A)

### Node under test

- Node: **Pi5 Node 1**
  - `node_id=0a55b329-104f-46f0-b50b-dea9a5cca1b3`
  - Renogy BT‑2 enabled (BLE)
  - PV forecast enabled (Forecast.Solar)
- True load sensor (watts; ADS1263-derived):
  - `sensor_id=af370b03a396ea19681af459` (`Node1 DC loads power`, unit `W`)

### Battery model config applied

- `PUT /api/battery/config/0a55b329-104f-46f0-b50b-dea9a5cca1b3`
  - enabled: `true`
  - sticker_capacity_ah: `100`
  - soc_cutoff_percent: `20`

### Power runway config applied

- `PUT /api/power/runway/config/0a55b329-104f-46f0-b50b-dea9a5cca1b3`
  - enabled: `true`
  - load_sensor_ids: `["af370b03a396ea19681af459"]`
  - pv_derate: `0.75`
  - history_days: `7`
  - projection_days: `5`

### Observed outputs (virtual sensors)

- Battery model virtual sensors present:
  - `99ca58094bd3c1f640535136` Battery SOC (est) (`%`)
  - `136dfbc1cd5fe03f460c9f76` Battery remaining (Ah) (`Ah`)
  - `2f9a3ecb4cd68889a197d63d` Battery capacity (est, Ah) (`Ah`)
- Power runway virtual sensors present:
  - `9d8416255765b71ff841eaa4` Power runway (conservative, hr) (`hr`)
  - `d4b77c9928d83269cf69b3ae` Power runway min SOC projected (%) (`%`)
- Latest runway observation (via `POST /api/alarm-rules/preview` on `9d8416255765b71ff841eaa4`):
  - observed_value: `119.4061 hr` (condition `runway < 72 hr` evaluated `passed=false`)

### Alarm rule (warning) created + evaluated

- Created alarm rule:
  - `POST /api/alarm-rules` → `id=8` name=`Node1 power runway < 72h` severity=`warning`
- Evaluation sanity:
  - `POST /api/alarm-rules/preview` returned `targets_evaluated=1` with `observed_value` present (no errors).
  - `GET /api/alarm-rules` shows `id=8` has `last_error=null` and `last_eval_at` populated.

## Tier A Screenshot Review (Hard Gate)

- [x] REVIEWED: `manual_screenshots_web/20260218_144116/setup_battery_runway.png`
- [x] REVIEWED: `manual_screenshots_web/20260218_144116/power_battery_runway.png`
- [x] REVIEWED: `manual_screenshots_web/20260218_144116/alarms_runway_rule.png`

### Visual checks (required)

- [x] PASS: `setup_battery_runway.png` shows Battery & runway config is present and saved for Pi5 Node 1.
- [x] PASS: `power_battery_runway.png` shows estimated battery SOC/remaining and conservative runway readbacks on the Power tab.
- [x] PASS: `alarms_runway_rule.png` shows the warning alarm rule `Node1 power runway < 72h` is present and enabled.

### Findings

- Battery SOC estimator + conservative power runway projection are working on the installed controller and can be alarmed via Alarm Rules without errors.

### Reviewer declaration

I viewed each screenshot listed above.

## Tier‑A screenshot gate (hard gate command)

- Command: `make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260218-tier-a-cs108-dw258-battery-runway-0.1.9.274-battery-runway-fix.md`
- Result: PASS

