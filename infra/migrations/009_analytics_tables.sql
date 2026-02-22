create table if not exists analytics_power_samples (
    recorded_at timestamptz not null,
    metric text not null,
    value double precision not null,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    primary key (recorded_at, metric)
);

select create_hypertable('analytics_power_samples', 'recorded_at', if_not_exists => true);

create index if not exists analytics_power_samples_metric_idx
    on analytics_power_samples (metric, recorded_at desc);

create materialized view if not exists analytics_power_hourly
with (timescaledb.continuous) as
select
    time_bucket('1 hour', recorded_at) as bucket,
    metric,
    avg(value) as avg_value,
    sum(value) as total_value,
    max(recorded_at) as last_recorded_at
from analytics_power_samples
group by bucket, metric
with no data;

create index if not exists analytics_power_hourly_metric_bucket_idx
    on analytics_power_hourly (metric, bucket);

do $$
begin
    perform add_continuous_aggregate_policy(
        'analytics_power_hourly',
        start_offset => interval '30 days',
        end_offset => interval '5 minutes',
        schedule_interval => interval '15 minutes'
    );
exception
    when undefined_function then
        null;
    when duplicate_object then
        null;
    when invalid_parameter_value then
        null;
end
$$;

create table if not exists analytics_water_samples (
    recorded_at timestamptz not null,
    metric text not null,
    value double precision not null,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    primary key (recorded_at, metric)
);

select create_hypertable('analytics_water_samples', 'recorded_at', if_not_exists => true);

create index if not exists analytics_water_samples_metric_idx
    on analytics_water_samples (metric, recorded_at desc);

create materialized view if not exists analytics_water_hourly
with (timescaledb.continuous) as
select
    time_bucket('1 hour', recorded_at) as bucket,
    metric,
    avg(value) as avg_value,
    sum(value) as total_value,
    max(recorded_at) as last_recorded_at
from analytics_water_samples
group by bucket, metric
with no data;

create index if not exists analytics_water_hourly_metric_bucket_idx
    on analytics_water_hourly (metric, bucket);

do $$
begin
    perform add_continuous_aggregate_policy(
        'analytics_water_hourly',
        start_offset => interval '30 days',
        end_offset => interval '5 minutes',
        schedule_interval => interval '15 minutes'
    );
exception
    when undefined_function then
        null;
    when duplicate_object then
        null;
    when invalid_parameter_value then
        null;
end
$$;

create table if not exists analytics_soil_samples (
    recorded_at timestamptz not null,
    metric text not null,
    value double precision not null,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    primary key (recorded_at, metric)
);

select create_hypertable('analytics_soil_samples', 'recorded_at', if_not_exists => true);

create index if not exists analytics_soil_samples_metric_idx
    on analytics_soil_samples (metric, recorded_at desc);

create materialized view if not exists analytics_soil_hourly
with (timescaledb.continuous) as
select
    time_bucket('1 hour', recorded_at) as bucket,
    metric,
    avg(value) as avg_value,
    sum(value) as total_value,
    max(recorded_at) as last_recorded_at
from analytics_soil_samples
group by bucket, metric
with no data;

create index if not exists analytics_soil_hourly_metric_bucket_idx
    on analytics_soil_hourly (metric, bucket);

do $$
begin
    perform add_continuous_aggregate_policy(
        'analytics_soil_hourly',
        start_offset => interval '30 days',
        end_offset => interval '5 minutes',
        schedule_interval => interval '15 minutes'
    );
exception
    when undefined_function then
        null;
    when duplicate_object then
        null;
    when invalid_parameter_value then
        null;
end
$$;

create table if not exists analytics_soil_field_stats (
    recorded_at timestamptz not null,
    field_name text not null,
    min_pct double precision not null,
    max_pct double precision not null,
    avg_pct double precision not null,
    metadata jsonb not null default '{}'::jsonb,
    primary key (recorded_at, field_name)
);

select create_hypertable('analytics_soil_field_stats', 'recorded_at', if_not_exists => true);

create index if not exists analytics_soil_field_stats_field_idx
    on analytics_soil_field_stats (field_name, recorded_at desc);

create table if not exists analytics_status_samples (
    recorded_at timestamptz not null,
    metric text not null,
    value double precision not null,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    primary key (recorded_at, metric)
);

select create_hypertable('analytics_status_samples', 'recorded_at', if_not_exists => true);

create index if not exists analytics_status_samples_metric_idx
    on analytics_status_samples (metric, recorded_at desc);

create materialized view if not exists analytics_status_hourly
with (timescaledb.continuous) as
select
    time_bucket('1 hour', recorded_at) as bucket,
    metric,
    avg(value) as avg_value,
    sum(value) as total_value,
    max(recorded_at) as last_recorded_at
from analytics_status_samples
group by bucket, metric
with no data;

create index if not exists analytics_status_hourly_metric_bucket_idx
    on analytics_status_hourly (metric, bucket);

do $$
begin
    perform add_continuous_aggregate_policy(
        'analytics_status_hourly',
        start_offset => interval '30 days',
        end_offset => interval '5 minutes',
        schedule_interval => interval '15 minutes'
    );
exception
    when undefined_function then
        null;
    when duplicate_object then
        null;
    when invalid_parameter_value then
        null;
end
$$;

create table if not exists analytics_integration_status (
    id bigserial primary key,
    category text not null,
    name text not null,
    status text not null,
    recorded_at timestamptz not null default now(),
    metadata jsonb not null default '{}'::jsonb
);

create index if not exists analytics_integration_status_category_idx
    on analytics_integration_status (category, recorded_at desc);

create table if not exists analytics_rate_schedules (
    id bigserial primary key,
    category text not null,
    provider text not null,
    current_rate double precision not null,
    est_monthly_cost double precision not null,
    details jsonb not null default '{}'::jsonb,
    recorded_at timestamptz not null default now()
);

create index if not exists analytics_rate_schedules_category_idx
    on analytics_rate_schedules (category, recorded_at desc);
