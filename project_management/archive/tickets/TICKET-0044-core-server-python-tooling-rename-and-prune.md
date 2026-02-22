# TICKET-0044: Core-server Python tooling rename and prune

**Status:** Open

## Description
The repo still contains `apps/core-server/` (Python/Poetry) which historically was the FastAPI core-server. Even though production runtime is Rust now, this directory still includes FastAPI/uvicorn deps and stale documentation, which is confusing new developers and contradicts the “Rust-first controller runtime” principle.

We should either:
1) Rename/move this directory so it is unambiguously **tooling-only**, and/or
2) Prune it to the smallest possible set of dependencies so it cannot be mistaken for a runnable service.

Goal: eliminate the “core-server is still Python” confusion without breaking required tooling (migrations/seed helpers) or CI.

## Scope
* [ ] Decide on the desired end state:
  - Option A: Rename `apps/core-server` → `apps/core-tooling` (or `tools/core-tooling`) and remove “core-server” naming ambiguity.
  - Option B: Keep path but aggressively prune deps + delete stale docs that describe a FastAPI runtime.
  - Option C: Remove the directory entirely by porting its remaining responsibilities to Rust (tracked as follow-up in TICKET-0045).
* [ ] Remove stale guidance that implies there is an `app/main.py` FastAPI server under `apps/core-server`.
* [ ] Reduce dependencies to only what is needed for the remaining tooling that still imports `app.config` / `app.models`.
* [ ] Update scripts/CI caching paths that currently assume `apps/core-server` exists.

## Acceptance Criteria
* [ ] New developer can no longer plausibly conclude that the production controller core-server is Python by browsing repo layout/docs.
* [ ] `apps/core-server/AGENTS.md` is either removed or updated to clearly state tooling-only (no FastAPI runtime), and does not describe non-existent files/routers.
* [ ] If the directory remains, `pyproject.toml` no longer includes `fastapi`/`uvicorn` unless there is a documented, active dev-only reason.
* [ ] Any scripts that currently import from `apps/core-server/app/*` still work (or are migrated) with explicit documentation.
* [ ] CI jobs that previously installed `apps/core-server` deps are updated to install only what they need.

## Notes
- This ticket is about repo clarity + dependency pruning. It should not change production behavior (controller runtime is already Rust).

