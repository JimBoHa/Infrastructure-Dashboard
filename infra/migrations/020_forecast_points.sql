create table if not exists forecast_points (
    id bigserial,
    provider text not null,
    kind text not null,
    subject_kind text not null,
    subject text not null,
    latitude double precision,
    longitude double precision,
    issued_at timestamptz not null,
    ts timestamptz not null,
    metric text not null,
    value double precision not null,
    unit text not null,
    metadata jsonb not null default '{}'::jsonb
);

select create_hypertable('forecast_points', 'ts', if_not_exists => true);

create index if not exists forecast_points_lookup_idx
    on forecast_points (provider, kind, subject_kind, subject, metric, ts desc);

create index if not exists forecast_points_issued_idx
    on forecast_points (provider, kind, subject_kind, subject, metric, issued_at desc);

create unique index if not exists forecast_points_unique_point_idx
    on forecast_points (provider, kind, subject_kind, subject, metric, issued_at, ts);
