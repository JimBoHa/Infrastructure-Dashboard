SHELL := /bin/bash
INFRA_DIR := infra
CORE_RS_DIR := apps/core-server-rs
NODE_DIR := apps/node-agent
DASH_DIR := apps/dashboard-web
CORE_DEMO_MODE ?= false
NEXT_PUBLIC_API_BASE ?= http://127.0.0.1:8000

.DEFAULT_GOAL := help

.PHONY: help up down logs core web seed migrate ci ci-full ci-smoke e2e-web-smoke e2e-setup-smoke e2e-setup-smoke-quarantine e2e-installed-health-smoke e2e-installer-stack-smoke e2e-installer-stack-smoke-quarantine
.PHONY: test-preflight test-clean test-postflight
.PHONY: rcs-parity-smoke rcs-openapi-coverage
.PHONY: ci-core ci-node ci-web ci-core-smoke ci-node-smoke ci-web-smoke ci-web-smoke-build demo-live
.PHONY: ci-presets-smoke
.PHONY: ci-farmctl ci-farmctl-smoke
.PHONY: ci-integrity-guardrail
.PHONY: tier-a-screenshot-gate
.PHONY: bootstrap
.PHONY: rust-sidecar

help:
	@echo "Available targets:"
	@echo "  make bootstrap - install app dependencies (poetry/npm)"
	@echo "  make up   - deprecated (legacy stack removed; use native services)"
	@echo "  make down - deprecated (legacy stack removed)"
	@echo "  make logs - deprecated (legacy stack removed)"
	@echo "  make rust-sidecar - run the Rust telemetry ingest sidecar"
	@echo "  make core - run Rust core-server (controller backend)"
	@echo "  make web  - run Next.js dashboard"
	@echo "  make migrate - apply SQL migrations"
	@echo "  make seed - seed demo data"
	@echo "  make ci   - run full local test suite (core-server, node-agent, dashboard)"
	@echo "  make ci-smoke - run fast smoke tests (core-server, node-agent, dashboard)"
	@echo "  make ci-web-smoke-build - run dashboard-web lint + smoke + build"
	@echo "  make ci-farmctl-smoke - run fast farmctl cargo tests"
	@echo "  make ci-integrity-guardrail - enforce production token guardrail allowlist"
	@echo "  make ci-full - run full test suite (core-server, node-agent, dashboard)"
	@echo "  make test-preflight - verify no orphaned Farm services/processes"
	@echo "  make test-clean - kill/bootout orphaned Farm services/processes"
	@echo "  make test-postflight - verify no orphaned services remain"
	@echo "  make e2e-web-smoke - run Sim Lab Playwright smoke against an installed bundle"
	@echo "  make e2e-setup-smoke - build installer DMG and validate install/upgrade/rollback"
	@echo "  make e2e-setup-smoke-quarantine - same as setup-smoke, but simulates a quarantined downloaded DMG"
	@echo "  make e2e-installed-health-smoke - fast non-UI health check of the installed stack"
	@echo "  make tier-a-screenshot-gate RUN_LOG=<project_management/runs/RUN-...md> - hard gate: screenshot review evidence in Tier-A run log"
	@echo "  make e2e-installer-stack-smoke - run setup-smoke then web smoke"
	@echo "  make e2e-installer-stack-smoke-quarantine - full stack smoke with quarantined installer DMG simulation"
	@echo "  make rcs-parity-smoke - compare Rust OpenAPI subset vs canonical spec"
	@echo "  make rcs-openapi-coverage - ensure Rust router covers OpenAPI contract"
	@echo "  make demo-live - run migrations/seed then launch core, telemetry-sidecar, web, and Sim Lab"

bootstrap:
	@if ! command -v poetry >/dev/null 2>&1; then echo "Poetry not installed; see docs/README.md"; exit 1; fi
	@if ! command -v npm >/dev/null 2>&1; then echo "npm not installed; install Node.js 20+ first"; exit 1; fi
	cd apps/node-agent && poetry install
	cd apps/dashboard-web && npm install

up:
	@echo "Legacy stack is deprecated. Use native services (farmctl/launchd)."; exit 1

# caution: destructive volume removal

down:
	@echo "Legacy stack is deprecated. Use native services (farmctl/launchd)."; exit 1

logs:
	@echo "Legacy stack is deprecated. Use native service logs."; exit 1

core:
	@if [ -z "$$CORE_DATABASE_URL" ]; then \
		echo "CORE_DATABASE_URL must be set (use an installed stack or export a local DB URL)."; \
		echo "Tip: run 'FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke' to stand up an isolated E2E stack."; \
		exit 1; \
	fi
	cd $(CORE_RS_DIR) && cargo run -- --host 127.0.0.1 --port 8000

web:
	cd $(DASH_DIR) && npm run dev -- --hostname 0.0.0.0

rust-sidecar:
	cd apps/telemetry-sidecar && cargo run

migrate:
	@DB_URL=$${CORE_DATABASE_URL:-$$DATABASE_URL}; \
	if [ -z "$$DB_URL" ]; then \
		echo "CORE_DATABASE_URL or DATABASE_URL must be set to apply migrations."; \
		echo "Tip: run 'FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke' for an isolated stack."; \
		exit 1; \
	fi; \
	cargo run --manifest-path apps/farmctl/Cargo.toml -- db migrate --database-url "$$DB_URL" --migrations-root infra/migrations

seed:
	@DB_URL=$${CORE_DATABASE_URL:-$$DATABASE_URL}; \
	if [ -z "$$DB_URL" ]; then \
		echo "CORE_DATABASE_URL or DATABASE_URL must be set to seed demo data."; \
		echo "Tip: run 'FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke' for an isolated stack."; \
		exit 1; \
	fi; \
	echo "WARNING: seeding is destructive. Do not run against a production database."; \
	cargo run --manifest-path apps/farmctl/Cargo.toml -- db seed-demo --database-url "$$DB_URL"

ci: ci-full

ci-full: ci-core ci-node ci-web-full

ci-smoke: ci-core-smoke ci-node-smoke ci-web-smoke

ci-farmctl:
	cargo test --manifest-path apps/farmctl/Cargo.toml

ci-farmctl-smoke: ci-farmctl

ci-integrity-guardrail:
	python3 tools/production_token_guardrail.py

test-preflight:
	python3 tools/test_hygiene.py

test-clean:
	python3 tools/test_hygiene.py --apply

test-postflight:
	python3 tools/test_hygiene.py

e2e-web-smoke:
	FARM_E2E_REQUIRE_INSTALLED=1 python3 tools/e2e_web_smoke.py

e2e-setup-smoke:
	python3 tools/e2e_setup_smoke.py

e2e-setup-smoke-quarantine:
	FARM_E2E_QUARANTINE_INSTALLER_DMG=1 python3 tools/e2e_setup_smoke.py

e2e-installed-health-smoke:
	FARM_E2E_REQUIRE_INSTALLED=1 python3 tools/e2e_installed_health_smoke.py

tier-a-screenshot-gate:
	@if [ -z "$(RUN_LOG)" ]; then \
		echo "RUN_LOG is required. Example:"; \
		echo "  make tier-a-screenshot-gate RUN_LOG=project_management/runs/RUN-20260210-tier-a-....md"; \
		exit 1; \
	fi
	python3 tools/tier_a_screenshot_gate.py --run-log "$(RUN_LOG)"

e2e-installer-stack-smoke:
	python3 tools/e2e_installer_stack_smoke.py

e2e-installer-stack-smoke-quarantine:
	FARM_E2E_QUARANTINE_INSTALLER_DMG=1 python3 tools/e2e_installer_stack_smoke.py

e2e-preset-flows-smoke:
	python3 tools/e2e_preset_flows_smoke.py

rcs-parity-smoke:
	python3 tools/rcs_parity_smoke.py

rcs-openapi-coverage:
	python3 tools/check_openapi_coverage.py

ci-core:
	python3 tools/check_integration_presets.py
	python3 tools/check_openapi_coverage.py
	cargo test --manifest-path apps/core-server-rs/Cargo.toml

ci-core-smoke:
	python3 tools/check_integration_presets.py
	python3 tools/check_openapi_coverage.py
	cargo test --manifest-path apps/core-server-rs/Cargo.toml

ci-presets-smoke:
	python3 tools/check_integration_presets.py

ci-node:
	python3 tools/node_offline_install_smoke.py
	cd $(NODE_DIR) && PYTHONPATH=. poetry run pytest

ci-node-smoke:
	python3 tools/node_offline_install_smoke.py
	cd $(NODE_DIR) && PYTHONPATH=. poetry run pytest -k smoke

ci-web:
	cd $(DASH_DIR) && CI=1 NEXT_PUBLIC_API_BASE=$(NEXT_PUBLIC_API_BASE) npm run lint && CI=1 NEXT_PUBLIC_API_BASE=$(NEXT_PUBLIC_API_BASE) npm run test

ci-web-smoke:
	cd $(DASH_DIR) && CI=1 NEXT_PUBLIC_API_BASE=$(NEXT_PUBLIC_API_BASE) npm run lint && CI=1 NEXT_PUBLIC_API_BASE=$(NEXT_PUBLIC_API_BASE) npm run test:smoke

ci-web-smoke-build: ci-web-smoke
	cd $(DASH_DIR) && CI=1 NEXT_PUBLIC_API_BASE=$(NEXT_PUBLIC_API_BASE) npm run build

ci-web-full: ci-web
	cd $(DASH_DIR) && CI=1 NEXT_PUBLIC_API_BASE=$(NEXT_PUBLIC_API_BASE) npm run build

demo-live:
	cd apps/node-agent && poetry run python ../../tools/sim_lab/run.py

.PHONY: adr ticket

adr: ## Create a new Architecture Decision Record. Usage: make adr t="Use External API"
	@chmod +x tools/create-adr.sh
	@./tools/create-adr.sh "$(t)"

ticket: ## Create a new detailed ticket file. Usage: make ticket t="Integrate AI Model"
	@chmod +x tools/create-ticket.sh
	@./tools/create-ticket.sh "$(t)"
