# 0009. Offline telemetry buffering: append-only segments + seq ACK + controller receipt-time liveness

* **Status:** Accepted
* **Date:** 2026-02-01
* **Implementation:** OT-1..OT-15 (`project_management/tickets/TICKET-0049-offline-telemetry-spool-+-backfill-replay-(append-only-segments-+-rust-node-forwarder-+-ack).md`)

## Context
Raspberry Pi nodes can temporarily lose connectivity to the controller (Wi‑Fi drop, switch reboot, broker/service restart, controller upgrade window). Today, this can result in:

- **Telemetry loss** (no durable buffering for multi-hour/day outages).
- **Sampling interruptions** (sampling/publish loops can stall when the uplink is hard down).
- **False offline flaps** if old samples are replayed with older timestamps and the controller uses `sample_ts` for liveness.

We want a design that is:
- robust to hard disconnects of **48h+**
- microSD friendly (sequential writes; bounded metadata churn)
- explicit about retention/drop policy (no “silent queue limit” drops)
- correct under MQTT QoS 1 (duplicates possible; at-least-once)

## Decision
Adopt **Option C**: an **append-only segment log spool** on each node plus a **Rust node-forwarder** that publishes live data and replays backlog on reconnect, with:

1) **Append-only, CRC-framed binary segments** on node storage
   - sequential writes only
   - crash recovery by scanning/truncating only the tail of the last `.open` segment
   - retention is explicit (`max_spool_bytes`, `max_spool_age`, drop-oldest-segments)

2) **Application-level ACK** from the controller to enable correct garbage collection
   - MQTT PUBACK confirms broker receipt, not DB durability
   - controller publishes `iot/{node_id}/ack` containing `acked_seq` (highest contiguous ingested seq)
   - node deletes only fully-ACKed closed segments

3) **Controller liveness computed from receipt time (received_at), not sample timestamps**
   - maintain monotonic `last_rx_at = max(received_at)` for node/sensor liveness
   - maintain monotonic `last_sample_ts = max(sample_ts)` for “data freshness”
   - replayed old timestamps must never regress “last seen”

4) **QoS 1 + idempotent ingest**
   - duplicates are expected under QoS 1
   - DB ingest remains idempotent via PK/unique constraint and `ON CONFLICT DO NOTHING` (or equivalent)

This ADR intentionally does **not** require MQTT v5 features for the MVP; it stays compatible with MQTT 3.1.1 semantics while allowing a future upgrade.

## Consequences
**Benefits**
- Nodes can buffer multi-day outages with bounded disk usage and deterministic drop policy.
- Spool is microSD-friendly (sequential appends + segment deletes).
- Replay is controllable (rate limiting, live/status priority).
- Controller liveness becomes stable under backfill (no flaps).

**Tradeoffs / risks**
- Adds a new node service (`node-forwarder`) and a local IPC boundary from Python → Rust.
- Requires controller-side ACK publishing and liveness semantics change.
- QoS 1 duplicates mean the ingest path must remain strictly idempotent (schema + code).
- Backfill at very large scale may exceed MQTT per-message throughput; Phase 2 may add HTTP bulk ingest.

**Alternatives considered**
- **A. Local Mosquitto + bridge store-and-forward:** simple, but queue caps can silently drop and retention semantics are broker-config dependent.
- **B. SQLite WAL spool + replay:** strong durability, but introduces more varied write patterns (WAL + b-tree) than pure append-only segments (microSD wear concern).
- **D. MQTT client persistence only:** designed for in-flight QoS delivery, not multi-day bounded time-series retention with drop/downsample policies.
- **E. NATS JetStream on node:** high capability but adds a second messaging stack and operational overhead.
