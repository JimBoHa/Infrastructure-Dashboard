-- Battery SOC + capacity estimation state (controller-side).
--
-- This table persists per-node estimator state so SOC can be coulomb-counted
-- between controller restarts and anchored/calibrated during low-current “rest”
-- windows.

CREATE TABLE IF NOT EXISTS battery_estimator_state (
  node_id UUID PRIMARY KEY,

  -- Current sign normalization:
  -- - current_sign = +1 => positive current increases SOC (charging)
  -- - current_sign = -1 => positive current decreases SOC (discharging)
  current_sign SMALLINT NOT NULL DEFAULT 1,
  sign_locked BOOLEAN NOT NULL DEFAULT FALSE,
  sign_votes_pos INTEGER NOT NULL DEFAULT 0,
  sign_votes_neg INTEGER NOT NULL DEFAULT 0,

  soc_est_percent DOUBLE PRECISION NOT NULL DEFAULT 0,
  capacity_est_ah DOUBLE PRECISION NOT NULL DEFAULT 0,

  -- Anchor/segment tracking for capacity estimation.
  last_anchor_soc_percent DOUBLE PRECISION,
  segment_ah_accumulated DOUBLE PRECISION NOT NULL DEFAULT 0,

  -- “Resting” detection: the timestamp when abs(current) first dropped below the rest threshold.
  rest_started_at TIMESTAMPTZ,

  -- Latest processed sample timestamp (used to compute dt for integration).
  last_ts TIMESTAMPTZ NOT NULL,

  -- For diagnostics/debugging.
  last_current_a DOUBLE PRECISION,
  last_voltage_v DOUBLE PRECISION,
  last_renogy_soc_percent DOUBLE PRECISION,

  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

