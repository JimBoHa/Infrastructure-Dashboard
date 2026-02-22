-- Support late-arrival replication into the TSSE Parquet analysis lake by tracking insertion time.
-- NOTE: Keep nullable to avoid table rewrite on large existing hypertables.

alter table if exists metrics
  add column if not exists inserted_at timestamptz;

create index if not exists metrics_inserted_at_idx on metrics(inserted_at);

