-- Chart annotations: persistent user-created or promoted-from-analysis markers
CREATE TABLE IF NOT EXISTS chart_annotations (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  chart_state   JSONB NOT NULL,
  sensor_ids    TEXT[] NULL,
  time_start    TIMESTAMPTZ NULL,
  time_end      TIMESTAMPTZ NULL,
  label         TEXT NULL,
  created_by    UUID NULL REFERENCES users(id) ON DELETE SET NULL,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS chart_annotations_created_by ON chart_annotations(created_by);
CREATE INDEX IF NOT EXISTS chart_annotations_time ON chart_annotations(time_start, time_end);
