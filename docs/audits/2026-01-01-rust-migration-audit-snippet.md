# Rust migration audit snippet (historical)

NOTE (2026-02-12): This file captured a point-in-time review. Since then:
- Legacy Python `apps/core-server` has been removed.
- The WAN portal scaffold has been removed.
- The canonical OpenAPI contract is exported to `apps/core-server-rs/openapi/farm-dashboard.json` and SDKs are generated from it (`tools/api-sdk/generate.py`).
- The old HTTP parity harness (`tools/rcs_parity_http_smoke.py`) has been removed.

Treat the remainder of this document as historical context, not current instructions.

## Pruned details (ARCH-6)

The original detailed notes in this snippet referenced codepaths and tooling that no longer exist in the repo
(Python core-server runtime, WAN portal scaffold, HTTP parity harness). The detail was removed as part of the
repo-wide pruning pass to avoid leaving misleading “how to run this” instructions behind.

For current migration status and contract coverage:
- ADR intent: `docs/ADRs/0004-rust-coreserver-migration-(api-+-static-dashboard-served-by-rust).md`
- Contract coverage: `python3 tools/check_openapi_coverage.py` (and `make rcs-parity-smoke`)
