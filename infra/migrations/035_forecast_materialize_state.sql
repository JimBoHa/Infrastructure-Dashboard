-- Track forecast materialization worker state
create table if not exists forecast_materialize_state (
  id serial primary key,
  watermark_ts timestamptz null,
  updated_at timestamptz not null default now()
);

-- Insert initial row
insert into forecast_materialize_state (id) values (1) on conflict do nothing;
