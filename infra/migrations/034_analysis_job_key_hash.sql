-- Avoid Postgres index/key-size limits for large dedupe keys.
--
-- The dashboard sometimes sends very large JSON job_key payloads (e.g. many sensor IDs).
-- A unique btree index on (job_type, job_key) can fail with large values, producing
-- `500 Database error` on POST /api/analysis/jobs.
--
-- Fix: store a fixed-size SHA-256 hash for dedupe/indexing.

ALTER TABLE analysis_jobs
  ADD COLUMN IF NOT EXISTS job_key_hash text;

-- Some historical installs may have a unique index on `(job_type, job_key_hash)` that was created
-- before the backfill completed, which can cause the backfill itself to fail. Prefer keeping
-- upgrades safe over enforcing uniqueness here.
DROP INDEX IF EXISTS analysis_jobs_type_job_key_hash_uniq;

-- Backfill existing rows (pgcrypto is already installed in 001_init.sql).
UPDATE analysis_jobs
SET job_key_hash = encode(digest(job_key, 'sha256'), 'hex')
WHERE job_key IS NOT NULL
  AND job_key_hash IS NULL;

-- Non-unique supporting index for best-effort dedupe lookups by hash.
CREATE INDEX IF NOT EXISTS analysis_jobs_type_job_key_hash_idx
  ON analysis_jobs(job_type, job_key_hash)
  WHERE job_key_hash IS NOT NULL;

-- Drop the old index that could fail on large text values.
DROP INDEX IF EXISTS analysis_jobs_type_job_key_uniq;
