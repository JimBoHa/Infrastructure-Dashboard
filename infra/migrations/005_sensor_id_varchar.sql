do $$
begin
    perform remove_continuous_aggregate_policy('metrics_agg_5m');
exception
    when undefined_function then
        null;
    when others then
        null;
end$$;

drop materialized view if exists metrics_agg_5m cascade;

alter table sensors
    alter column sensor_id type varchar(24)
    using trim(sensor_id);

alter table metrics
    alter column sensor_id type varchar(24)
    using trim(sensor_id);

alter table alarms
    alter column sensor_id type varchar(24)
    using trim(sensor_id);

create materialized view if not exists metrics_agg_5m
with (timescaledb.continuous) as
select
    time_bucket('5 minutes', ts) as bucket,
    sensor_id,
    avg(value) as avg_value,
    avg(quality) as avg_quality,
    count(*) as samples
from metrics
group by bucket, sensor_id
with no data;

create index if not exists metrics_agg_5m_sensor_bucket_idx
    on metrics_agg_5m (sensor_id, bucket);

do $$
begin
    perform add_continuous_aggregate_policy(
        'metrics_agg_5m',
        start_offset => interval '30 days',
        end_offset => interval '5 minutes',
        schedule_interval => interval '5 minutes'
    );
exception
    when duplicate_object then
        null;
    when invalid_parameter_value then
        null;
end$$;
