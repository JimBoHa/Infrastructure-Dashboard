# TICKET-0049: Offline telemetry spool + backfill replay (append-only segments + Rust node-forwarder + ACK)

**Status:** Closed (Implemented; Tier A + hardware validated; Tier B OT-13 deferred)

## Description
Nodes must remain useful during hard disconnects from the controller (hours to days) without silently losing telemetry or flapping “offline” status when backfill occurs. The selected architecture is **Option C: append-only segment spool on each node + Rust node-forwarder that publishes live + replays backlog at a controlled rate**, plus a controller **application-level ACK** so the node can safely delete segments only after durable DB ingest. Controller liveness must be computed from **receipt time**, not `sample_ts`, and the controller must track separate clocks (`last_rx_at`, `last_sample_ts`) so replaying old timestamps does not regress “last seen”.

This ticket is the long-form requirements/spec dump for OT (Offline Telemetry) work. Implementation is tracked by `project_management/TASKS.md` (OT-*).

## Chosen Approach (Option C)
**Node:**
- `node-agent` (Python) always samples on schedule and emits normalized samples over local IPC (UDS preferred) to:
- `node-forwarder` (Rust) which:
  1) appends each sample to an **append-only, binary, CRC-framed segment log** on disk
  2) publishes samples live (low latency) when connected
  3) replays backlog in **capture order** on reconnect (rate-limited)
  4) truncates spool only when controller ACKs durable ingestion (delete whole closed segments)

**Controller:**
- Mosquitto broker receives telemetry over MQTT.
- `telemetry-sidecar` writes samples to TimescaleDB and publishes periodic `iot/{node_id}/ack` with `acked_seq` (highest contiguous ingested sequence).
- Core server liveness uses **receipt time** (or sidecar-reported `received_at`) and is monotonic; `sample_ts` is used only for data freshness.

## Raw Sizing Math (Baseline)
Assuming 40 bytes/sample on disk (32B payload + 8B frame header):
- Channels = 10
- Rate = 1 Hz
- Duration = 48h = 172,800s
- Samples = 10 × 1 × 172,800 = 1,728,000
- Bytes = 1,728,000 × 40 = 69,120,000 B = 65.93 MiB
- Catch-up time at 2,000 publishes/s = 864s = 14.4 minutes

## Requirements
### Durability + crash recovery
- Spool is append-only. Frames are length-delimited with CRC so the tail can be recovered after power loss.
- Writer uses periodic `fdatasync()` batching (default ~1s) rather than per-sample `fsync()`.
- Segment rotation supports both time and size rollovers (default: 1h OR 128 MiB).
- On startup, `node-forwarder` must scan only the most recent `.open` segment, truncate to the last valid frame boundary, and continue.

### Bounded storage + deterministic drop policy
- Spool has a **hard cap** (`max_spool_bytes`) and optional **age cap** (`max_spool_age`).
- When caps are hit: delete **oldest closed segments** until under cap.
- Data loss must be surfaced as a durable “loss event” (range + estimated count) and included in node status so operators know what was dropped.

### Replay behavior
- Replay in capture order: segment order then frame order (no global timestamp sorting).
- Rate-limited replay:
  - `replay_msgs_per_sec` default 2,000
  - `replay_bytes_per_sec` default TBD (2–10 MB/s initial target)
- Replay must not starve live status:
  - status/heartbeat publish path must remain periodic and not queue behind replay.

### Correct spool truncation (requires application ACK)
- MQTT QoS 1 PUBACK only means “broker received”, not “DB committed”.
- Controller must publish `iot/{node_id}/ack` including:
  - `acked_seq`: highest contiguous sequence number persisted in Timescale
  - optional debug fields (`acked_sample_ts`, `boot_id`, etc.)
- Node deletes only closed segments fully <= `acked_seq`.

### Controller liveness semantics (required change)
Backfill replays old timestamps by design; therefore:
- Online/offline must be computed from **controller receipt time**, not `payload.timestamp`.
- Track two separate monotonic clocks per node/sensor:
  - `last_rx_at = max(received_at)` for liveness
  - `last_sample_ts = max(sample_ts)` for “data freshness”
- Derived state:
  - `node_online = (now - node_last_rx_at) < ONLINE_TIMEOUT`
  - `sensor_data_fresh = (now - sensor_last_sample_ts) < FRESHNESS_TIMEOUT`

### Duplicates + idempotency
- Use MQTT QoS 1 for telemetry; duplicates are allowed.
- DB ingest must be idempotent: inserts are `ON CONFLICT DO NOTHING` (or equivalent) on the series primary key (typically `(sensor_id, sample_ts)`).

### Observability / operator UX
Node status must expose at least:
- spool bytes, closed/open segment counts, oldest sample age
- last acked sequence, last published sequence, drop counters/ranges
- replay state: idle vs draining, current throttle configuration, estimated drain time

Controller must expose:
- ack progression per node
- ingest lag / queue health under replay load
- liveness that remains stable during replay (no flaps)

## Decisions (resolved in implementation)
- **Disk budget per node:** default cap policy is `min(max(1GiB, 5% of filesystem), 25GiB)` with a **keep-free floor** (default `2GiB`). Overrides via `NODE_FORWARDER_MAX_SPOOL_BYTES` / `NODE_FORWARDER_KEEP_FREE_BYTES`.
- **Time accuracy requirement:** Phase 1 assumes **±5s over 48h is acceptable** and uses `time_quality` + monotonic timestamps; Phase 2 time anchors/drift correction remains optional (OT-15).
- **Sampling model:** Phase 1 targets periodic/control telemetry (1Hz+). High-rate/bursty payload classes (audio/vibration) remain out of scope unless promoted to a requirement.
- **Security posture:** Phase 1 assumes LAN trust; spool is protected via filesystem permissions/service user separation (no at-rest encryption). Device-capture-in-scope remains Phase 2.
- **MQTT v3.1.1 vs v5:** MVP remains compatible with MQTT **3.1.1 semantics**; MQTT v5 metadata improvements are optional follow-up.

## Environment Notes (current nodes; for sizing sanity)
- Pi5 Node 1 boots from **microSD** (~119 GB disk; ~105 GB free on `/` at last check).
- Pi5 Node 2 boots from **NVMe** (CT500P310SSD8; ~458 GB disk; ~433 GB free on `/` at last check).
- Both nodes have `systemd-timesyncd` active and a Raspberry Pi RTC device (`/dev/rtc0`).

## Scope
- [x] Define the exact telemetry envelope (fields, seq/stream_id, time_quality, backfill flag).
- [x] Implement node-forwarder (Rust) spool writer + recovery + replay publisher + rate limiting.
- [x] Modify node-agent sampling to always sample and send via local IPC (stop tying sampling to MQTT connectivity).
- [x] Implement controller ACK topic and monotonic liveness semantics (`last_rx_at` vs `last_sample_ts`).
- [x] Add integration/E2E harness for disconnect/reconnect and reboot-during-outage.
- [x] Prune/replace legacy node offline buffer codepaths so there is one clear durability layer.

## Acceptance Criteria
- [x] Simulated/forced hard-disconnect retains samples up to the configured cap; if cap exceeded, drop-oldest triggers and a loss-range event is emitted.
- [x] Reconnect drains backlog at configured rate limits without destabilizing broker/DB; live status/heartbeat remains responsive during drain.
- [x] No controller offline flaps during backlog replay (liveness computed from receipt time and monotonic).
- [x] Reboot mid-outage does not corrupt spool; forwarder recovers by truncating partial tail frame only.
- [x] Tier A validated on installed controller (no DB/settings reset) with evidence recorded; clean-host Tier B tracked via OT-13.
- [x] Hardware validation completed on Pi 5 nodes (microSD + NVMe) with disconnect + reboot-mid-outage scenario.

## Notes
- ADR: `docs/ADRs/0009-offline-telemetry-buffering:-append-only-segments-+-seq-ack-+-controller-receipt-time-liveness.md`
- Prompt used for external architecture review: `docs/prompts/gpt52pro_offline_buffering_architecture_prompt.txt`
- Tier A + hardware evidence: `project_management/runs/RUN-20260201-tier-a-ot49-offline-buffering-0.1.9.234-ot49.md`
