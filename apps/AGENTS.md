# Application-Specific Agent Instructions

This directory contains the source code for the individual applications that make up the Farm Dashboard platform.
Each application has its own dedicated `AGENTS.md` file with specific technical context, architectural patterns, and validation status.

**Agents must consult the specific AGENTS.md file for the application they are working on.**

## Index

- **Backend / API (Production):** [Rust Core Server Instructions](core-server-rs/AGENTS.md)
- **Frontend / Web:** [Dashboard Web Instructions](dashboard-web/AGENTS.md)
- **IoT / Linux:** [Node Agent Instructions](node-agent/AGENTS.md)
- **Tooling / Packaging:** [farmctl Instructions](farmctl/AGENTS.md)
- **Setup / Installer:** [Setup App Instructions](setup-app/AGENTS.md)

## Universal Rules
Refer to the root [AGENTS.md](../AGENTS.md) for global project constraints, including:
1.  **Git Safety:** No `git restore` without review.
2.  **Single Source of Truth:** `project_management/TASKS.md` for tracked implementation/product work; documentation-only edits do not require creating new tasks unless explicitly requested.
3.  **CI Policy:** The pre-commit hook uses `tools/git-hooks/select-tests.py` to run lightweight CI smoke targets for touched components (`make ci-smoke`, `make ci-core-smoke`, `make ci-node-smoke`, `make ci-web-smoke`, `make ci-farmctl-smoke`).
