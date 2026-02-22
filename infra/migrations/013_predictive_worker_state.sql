-- Track predictive worker cursor for DB-driven inference
create table if not exists predictive_worker_state (
  id serial primary key,
  last_ts timestamptz null,
  last_sensor_id varchar(24) null,
  updated_at timestamptz not null default now()
);
