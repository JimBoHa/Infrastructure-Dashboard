create table if not exists forecast_data (
    id bigserial primary key,
    field text not null,
    horizon_hours int not null default 24,
    value double precision not null,
    recorded_at timestamptz not null default now()
);

create index if not exists forecast_data_field_idx
    on forecast_data (field, horizon_hours, recorded_at desc);

create table if not exists analytics_indicators (
    id bigserial primary key,
    key text not null,
    value double precision not null,
    context jsonb not null default '{}'::jsonb,
    recorded_at timestamptz not null default now()
);

create index if not exists analytics_indicators_key_idx
    on analytics_indicators (key, recorded_at desc);
