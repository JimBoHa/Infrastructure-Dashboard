# RUN-20260123: TSSE collab-harness implementation (WIP)

Date: 2026-01-23

Goal: Implement the full TSSE ticket set (TSE-0001..TSE-0022) and complete TSSE-1 Tier‑A validation on the installed controller (no DB/settings reset), with captured-and-reviewed screenshot evidence.

Repo scope: `project_management/archive/archive/tickets/TSSE-INDEX.md`

---

## Collab Harness (multi-agent) roster

Orchestrator: Codex (this run)

Workers (explicit roles + deliverables):
- Worker A — Data plane: Parquet lake spec/impl, replication/backfill/compaction, DuckDB query service, NAS cold partitions.
- Worker B — Algorithms: embeddings/features, Qdrant candidate generation, exact episodic scoring, related_sensors_v1 + preview endpoints.
- Worker C — Dashboard UX: Trends “Related sensors” job UX, progress/cancel, episodic results + preview drilldown, tests/guardrails.
- Worker D — Ops/Security/Bench: perf/scale benchmarks, observability/why-ranked, security hardening (paths/perms/authz/abuse limits), Tier‑A validation checklist.

Review checkpoints (must be explicitly acknowledged in this file before marking TSSE tickets Done):
1) Data-plane acceptance review (TSE-0004/0005/0006/0021)
2) ANN + scoring acceptance review (TSE-0008/0009/0010)
3) Related Sensors job E2E + preview acceptance review (TSE-0011/0012)
4) Dashboard job UX acceptance review (TSE-0013)
5) Bench/obs/security acceptance review (TSE-0019/0020/0022)
6) Full suite green (`make ci-smoke` at minimum; `make ci` preferred) + Tier‑A validation evidence reviewed

---

## Commands run (append-only)

> Add commands + final pass/fail only (no streaming logs).

- 2026-01-23: `python3 tools/check_openapi_coverage.py` (FAIL — extra route `POST /api/analysis/preview`)
- 2026-01-23: `python3 tools/check_openapi_coverage.py` (PASS)
- 2026-01-23: `make ci-core-smoke` (PASS)
- 2026-01-23: `make ci-web-smoke` (PASS)
- 2026-01-23: `make ci-smoke` (PASS)
- 2026-01-23: `make ci-farmctl-smoke` (PASS)
- 2026-01-23: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS)
- 2026-01-23: `make ci-core-smoke` (PASS)
- 2026-01-24: `make ci-web-smoke-build` (PASS)
- 2026-01-24: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS; log: `reports/cargo-test-core-server-rs-20260123_174724.log`)
- 2026-01-24: `make ci-smoke` (PASS; log: `reports/ci-smoke-20260123_174943.log`)
- 2026-01-24: `make ci-full` (FAIL; iOS step blocked: `xcrun simctl` unavailable because Xcode is not installed/selected; log: `reports/ci-full-20260123_175025.log`)
- 2026-01-24: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS; includes bounded `matrix_profile_v1` compute)
- 2026-01-24: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS; timings alias follow-ups)
- 2026-01-24: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS; lake inserted_at + union_by_name reads)
- 2026-01-24: `make ci-smoke` (PASS)
- 2026-01-24: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS; log: `reports/cargo-test-core-server-rs-20260124_022837.log`)
- 2026-01-24: `cargo test --manifest-path apps/farmctl/Cargo.toml` (PASS; log: `reports/cargo-test-farmctl-20260124_022838.log`)
- 2026-01-24: `make ci-web-smoke-build` (PASS; log: `reports/ci-web-smoke-build-20260124_022839.log`)
- 2026-01-24: `make ci-smoke` (PASS; local validation before Tier‑A upgrade)
- 2026-01-24: `FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:3005 npx playwright test --project=chromium-desktop playwright/mobile-audit.spec.ts playwright/mobile-shell.spec.ts playwright/overview-mermaid-tooltips.spec.ts playwright/trends-auto-compare.spec.ts playwright/trends-relationships.spec.ts playwright/trends-event-match.spec.ts playwright/trends-cooccurrence.spec.ts playwright/trends-matrix-profile.spec.ts` (PASS; 24 tests)
- 2026-01-24: `cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_recall_eval -- --input-mode api --api-base-url http://127.0.0.1:8000 --auth-token-file /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt --qdrant-url http://127.0.0.1:6333 --interval-seconds 30 --candidate-limit 150 --min-pool 150 --pairs-file reports/tsse-recall-renogy-voltage-V-pairs.csv --sensor-ids-file reports/tsse-recall-renogy-voltage-V-sensor-ids.txt --unit V --focus-sample 3 --min-mean-recall 0.8 --report reports/tsse-recall-eval-renogy-voltage-V-pairs-20260124_165326-k150-0.1.9.211.md` (PASS; curated-pairs recall evidence)
- 2026-01-24: `cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin tsse_lake_inspector -- --mode api --api-base-url http://127.0.0.1:8000 --auth-token-file /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt > reports/tsse-lake-inspect-20260124_165836-0.1.9.211.json` (PASS; replication/backfill watermark evidence)
- 2026-01-24: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS; post-evidence tool tweaks)
- 2026-01-24: `make ci-smoke` (PASS; post-evidence tool tweaks)
- 2026-01-24: `make e2e-installed-health-smoke` (PASS; re-validated installed controller health without DB reset)

---

## Worker checkpoints (append-only)

- 2026-01-23: Worker A: implementing lake manifest metadata + compaction signals + hot/cold inspection + partition move tool + pruning tests (ETA 2–3h).
- 2026-01-23: Worker B: confirmed TSSE stubs remain; starting embeddings → candidate gen → scoring → related job + preview improvements + tests (ETA multi-session).
- 2026-01-23: Worker C: started dashboard job-UX refactor for AutoComparePanel; added `apps/dashboard-web/src/types/analysis.ts` types; next wire API helpers + progress polling + preview drilldown + tests (ETA 3–5h).
- 2026-01-23: Worker D: identified missing bench/obs/security work; starting security hardening + phase timings + benchmark harness + tests (ETA multi-session).
- 2026-01-23: Worker D: drafted TSSE bench report template under `reports/` and enumerated security test gaps (job caps, preview max window) plus Tier-A evidence expectations.
- 2026-01-24: Worker A: implemented TSSE-24 DuckDB correctness tests (multi-sensor points across partitions/files; bucket alignment + samples) in `apps/core-server-rs/src/services/analysis/parquet_duckdb.rs` and validated with `cargo test`.
- 2026-01-24: Worker B: delivered recall evaluation harness `apps/core-server-rs/src/bin/tsse_recall_eval.rs` (curated/synthetic pairs; report under `reports/`) to support TSE-0008/0009 acceptance.
- 2026-01-24: Worker C: delivered parity spot-check runbook `docs/runbooks/tsse-lake-backfill-and-parity.md` + CLI `apps/core-server-rs/src/bin/tsse_lake_parity_check.rs` to support TSE-0005 operator verification.
- 2026-01-24: Orchestrator: spawned Collab Harness workers for completion push — Worker A (data plane/backfill/parity), Worker B (Qdrant/embeddings/candidate-gen + observability), Worker C (dashboard TSSE UX + Playwright), Worker D (bench harness + CI fixes). Deliverables tracked in this run log; no worker may write under `project_management/` or run git state-changing commands.
- 2026-01-24: Orchestrator: continuation session — re-spawned workers for final TSSE closure:
  - Worker A (`019bef50-4a9b-7a73-8b94-59260d9ff272`): replication/backfill/parity/compaction acceptance + evidence commands.
  - Worker B (`019bef50-533c-74c0-81ca-2c8fdbef8edc`): embeddings refresh scheduling + recall report + candidate-gen filter/index review.
  - Worker C (`019bef50-5d4c-7492-b3e3-06b5406c0345`): dashboard TSSE UX gaps + Playwright stability + progress/cancel assertions.
  - Worker D (`019bef50-67e7-7e40-bbad-8375c6bb34b9`): observability (timings/why_ranked/profiling) + TSSE bench gates + Tier‑A checklist.
- 2026-01-24: Orchestrator: continuation session (post-interrupt) — re-spawned workers for final TSSE completion on `tsse/complete`:
  - Worker A (`019bef77-f303-78a0-a162-497000d1dde4`): Qdrant launchd/upgrade gap (qdrant missing after Tier‑A upgrade); ensure qdrant runs on installed controller without DB reset.
  - Worker B (`019bef77-f9cd-71c0-bcb4-2bdc7fd77745`): fix API token auth path causing `POST /api/analysis/jobs` to 500 (“invalid user id”) on the installed controller.
  - Worker C (`019bef78-0241-74b0-9630-8ce09529e1d8`): close dashboard TSSE UX gaps (Relationships/MatrixProfile params + metadata) + strengthen Playwright (incl chromium-desktop project).
  - Worker D (`019bef78-07cd-7701-92c2-d4c770d06066`): replication acceptance gaps (bulk export/COPY path + `analysis_late_window_hours` usage) and evidence commands.
- 2026-01-24: Orchestrator: continuation session (resume after user interrupt) — spawned workers for final TSSE closure on `tsse/complete`:
  - Worker A (`019bef9f-0df4-7492-88de-425e7a4f7611`): fix embeddings/Qdrant point-id mapping blocker (UUIDv5 ids + payload sensor_id mapping) + tests.
  - Worker B (`019bef9f-19bd-7c43-ab4b-6f037d1adb1c`): audit remaining TSSE-* tasks + linked TSE-* tickets; report missing acceptance evidence.
  - Worker C (`019bef9f-2856-7540-93e6-a995aa58c16a`): verify Playwright desktop chromium project + TSSE UI schema drift; patch if needed.
  - Review checkpoints: (1) merge Worker A patch + run `cargo test` locally; (2) run `tsse_*` jobs on installed controller and verify Qdrant points_count > 0; (3) collect + visually review Tier‑A screenshots; (4) close TSSE tasks in `project_management/` only after evidence is present.
- 2026-01-24: Orchestrator: TSSE completion push (Collab Harness) on `tsse/complete` — spawned workers for targeted closure:
  - Worker A (`019beff2-b381-7962-820e-3535e28fcb48`): dashboard Matrix Profile parity + Playwright verification (no code changes required; confirmed params/results/watermarks already aligned; Playwright `trends-matrix-profile` passes under `chromium-desktop`).
  - Worker B (`019beff2-c655-79d2-9725-76474183a9cd`): matrix_profile_v1 compute budget/early-stop + warnings + tests.
  - Worker C (`019beff2-dd3a-7310-a383-6c3f85d22ff4`): profiling hook + timings_ms key standardization + tsse_bench CLI flags.
  - Worker D (`019beff2-ef6b-74e2-942b-8765b8946474`): replication bulk export/COPY path to remove row-by-row export; backfill staging updated.
  - Worker E (`019beff3-1a97-75f3-b4ee-958a13f3132e`): AutoComparePanel computed-through watermark + Playwright assertions for progress/cancel/watermark.
- 2026-01-24: Orchestrator: fixed Playwright `mobile-shell.spec.ts` node-card selector (avoid selecting the outer “Nodes” card) and made the sensor-row click robust; reran TSSE Playwright subset (`chromium-desktop`) and `make ci-smoke` (both PASS).
- 2026-01-24: Orchestrator: commits pushed to `tsse/complete`: `2c6542c` (core-server-rs TSSE perf/replication/profiling/matrix-profile budget) and `8e12fcd` (dashboard-web TSSE panels + Playwright coverage).
- 2026-01-24: Orchestrator: continuation session — spawned workers for acceptance/audit closure on `tsse/complete`:
  - Worker A (`019bf0e0-5163-7cf1-a3e1-528e347cc245`): replication/backfill/parity acceptance audit + evidence gaps list.
  - Worker B (`019bf0e0-5731-72f1-97f6-f8457095a97f`): embeddings/candidate-gen acceptance audit; confirmed scheduled embeddings refresh exists; flagged curated-pairs evidence gap (resolved via new recall report).
  - Worker C (`019bf0e0-5ef2-7a22-8b56-ee4353fb0e45`): UI/schema audit (agent timed out; no changes).

---

## Evidence (append-only)

> Tier‑A evidence must include: installed bundle version, and at least one screenshot path under `manual_screenshots_web/` that was opened and visually reviewed.

- Pending.
- 2026-01-24: Tier‑A refresh completed to `0.1.9.206` (from `0.1.9.205`), installed health smoke PASS, TSSE screenshots captured under `manual_screenshots_web/20260124_045442/`, bench report written at `reports/tsse-bench-20260124_050332-0.1.9.206.md` (PASS for candidate+preview thresholds). Evidence log: `project_management/runs/RUN-20260124-tier-a-tsse-0.1.9.206.md`. Screenshot review still pending (must open/view).
- 2026-01-24: Lake backfill attempt failed mid-run (`lake_backfill_v1`: DuckDB COPY output dir not empty when reusing a shared run dir). Fix landed in `52eb231` (write backfill day outputs to per-day dirs and make `tsse_lake_backfill` fail fast on job failure). Tier‑A rerun required.
- 2026-01-24: Tier‑A refresh completed to `0.1.9.211` (from `0.1.9.210`), installed health smoke PASS, Qdrant health PASS, and TSSE Playwright Tier‑A suite PASS (`chromium-desktop`; log: `reports/playwright-tier-a-tsse-20260124_082818-0.1.9.211.log`). TSSE screenshots captured under `manual_screenshots_web/tier_a_0.1.9.211_trends_*` and bench/recall/parity evidence recorded under `reports/` (0.1.9.211). Evidence log: `project_management/runs/RUN-20260124-tier-a-tsse-0.1.9.211.md`. Screenshot review still pending (must open/view to satisfy TSSE-1).
