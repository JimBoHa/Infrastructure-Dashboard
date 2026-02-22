-- Durable analysis jobs + events + results (TSSE / analytics plane)

create table if not exists analysis_jobs (
  id uuid primary key,
  job_type text not null,
  status text not null,
  job_key text null,
  created_by uuid null references users(id) on delete set null,
  params jsonb not null default '{}'::jsonb,
  progress jsonb not null default '{}'::jsonb,
  error jsonb null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  started_at timestamptz null,
  completed_at timestamptz null,
  cancel_requested_at timestamptz null,
  canceled_at timestamptz null,
  expires_at timestamptz null
);

create index if not exists analysis_jobs_status_idx on analysis_jobs(status);
create index if not exists analysis_jobs_type_idx on analysis_jobs(job_type);
create index if not exists analysis_jobs_created_by_idx on analysis_jobs(created_by);

-- Avoid Postgres index/key-size limits for large dedupe keys.
-- The dashboard may send very large JSON `job_key` payloads (e.g. many sensor IDs).
-- A unique btree index on `(job_type, job_key)` can fail with large values.
--
-- Fix: store a fixed-size SHA-256 hash for dedupe/indexing.
ALTER TABLE analysis_jobs
  ADD COLUMN IF NOT EXISTS job_key_hash text;

-- Older installs may have attempted to enforce uniqueness on `(job_type, job_key_hash)` before
-- the hash backfill was complete, which can make the backfill itself fail. We prefer
-- best-effort dedupe (app-level) over hard uniqueness to keep upgrades safe.
DROP INDEX IF EXISTS analysis_jobs_type_job_key_hash_uniq;

UPDATE analysis_jobs
SET job_key_hash = encode(digest(job_key, 'sha256'), 'hex')
WHERE job_key IS NOT NULL
  AND job_key_hash IS NULL;

-- Non-unique supporting index for best-effort dedupe lookups by hash.
CREATE INDEX IF NOT EXISTS analysis_jobs_type_job_key_hash_idx
  ON analysis_jobs(job_type, job_key_hash)
  WHERE job_key_hash IS NOT NULL;

DROP INDEX IF EXISTS analysis_jobs_type_job_key_uniq;

create table if not exists analysis_job_events (
  id bigserial primary key,
  job_id uuid not null references analysis_jobs(id) on delete cascade,
  kind text not null,
  payload jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);

create index if not exists analysis_job_events_job_id_id_idx
  on analysis_job_events(job_id, id);

create table if not exists analysis_job_results (
  job_id uuid primary key references analysis_jobs(id) on delete cascade,
  result jsonb not null,
  created_at timestamptz not null default now()
);
