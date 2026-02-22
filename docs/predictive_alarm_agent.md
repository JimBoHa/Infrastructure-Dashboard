# Predictive alarms (overview)

Predictive anomaly alarms are **optional** and disabled by default in production. When enabled, the controller can call an external inference service to score telemetry windows and emit predictive alarms.

Decision record:
- `docs/ADRs/0001-external-ai-for-anomaly-detection.md`

Implementation entrypoints:
- Rust API routes: `apps/core-server-rs/src/routes/predictive.rs`
- Setup Center stores credentials under `setup_credentials` (name: `predictive_alarms`)

## Notes

- Treat predictive tokens like passwords; keep them out of source control and logs.
