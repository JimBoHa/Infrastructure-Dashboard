# 0001. Use External API for Predictive Anomaly Detection

* **Status:** Proposed
* **Date:** 2025-12-15

## Context

The current farm monitoring platform relies solely on static threshold alarms (e.g., “Temperature > 80°F”). We want to introduce predictive capabilities to detect subtle anomalies before they become critical failures.

We considered two main approaches:

1. **Edge/Local inference:** Run a small TensorFlow Lite or ONNX model directly on `apps/core-server-rs` (Mac mini controller) or `apps/node-agent` (Raspberry Pi nodes).
2. **Remote inference:** Send sensor telemetry to an external AI model API for analysis.

## Decision

Integrate an external AI Model API into `apps/core-server-rs` to handle anomaly detection.

## Rationale

- **Resource constraints:** The node-agent hardware (Pi) has limited compute headroom and is critical for real-time sensor I/O. Loading it with inference tasks risks destabilizing the control loop.
- **Complexity:** Managing the lifecycle (training, versioning, deploying) of custom ML models locally adds significant operational overhead to the DevOps pipeline.
- **Velocity:** Using a managed API allows us to prototype and iterate on the predictive capability without maintaining the underlying ML infrastructure.
- **Centralization:** The core server already acts as the data aggregator. It is the logical place to batch data and communicate with an upstream API, keeping edge nodes “dumb” and resilient.

## Consequences

- **Positive:** Keeps the node-agent lightweight and reduces immediate maintenance burden for the team.
- **Negative:** Introduces a dependency on internet connectivity for advanced alarms (standard thresholds must still work offline) and introduces variable operational cost (API usage fees).
- **Mitigation:** Design the system to “fail open” — if the internet is down or the API fails, standard threshold alarms must continue to function without interruption.
