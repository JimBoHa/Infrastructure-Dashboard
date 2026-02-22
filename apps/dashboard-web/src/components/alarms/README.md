# Alarms UI (Scaffold)

Predictive alarms UI work should live under:

- `src/types/alarms.ts`: shared alarm fields (including predictive metadata).
- `src/lib/alarms/*`: alarm helpers (origin detection, filtering, formatting).
- `src/components/alarms/*`: reusable UI pieces for alarm origin and anomaly score.

This keeps the rest of the dashboard pages from embedding ad-hoc alarm parsing logic.
