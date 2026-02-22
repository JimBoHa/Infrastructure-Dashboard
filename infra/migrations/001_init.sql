create extension if not exists timescaledb;
create extension if not exists pgcrypto;

create table if not exists nodes (
id uuid primary key default gen_random_uuid(),
name text not null,
mac_eth macaddr,
mac_wifi macaddr,
ip_last inet,
status text not null default 'unknown',
uptime_seconds bigint default 0,
cpu_percent real default 0,
storage_used_bytes bigint default 0,
last_seen timestamptz,
config jsonb default '{}'::jsonb,
created_at timestamptz not null default now(),
unique(mac_eth, mac_wifi)
);

create table if not exists sensors (
sensor_id varchar(24) primary key,
node_id uuid not null references nodes(id) on delete cascade,
name text not null,
type text not null,
unit text not null,
interval_seconds int not null,
rolling_avg_seconds int default 0,
config jsonb default '{}'::jsonb,
created_at timestamptz not null default now(),
deleted_at timestamptz
);

create table if not exists metrics (
sensor_id varchar(24) not null references sensors(sensor_id) on delete cascade,
ts timestamptz not null,
value double precision not null,
quality smallint default 0,
primary key (sensor_id, ts)
);
select create_hypertable('metrics', 'ts', if_not_exists => true);

create table if not exists alarms (
id bigserial primary key,
name text not null,
sensor_id varchar(24),
node_id uuid,
rule jsonb not null,
status text not null default 'ok',
last_fired timestamptz
);

create table if not exists schedules (
id bigserial primary key,
name text not null,
rrule text not null,
conditions jsonb default '[]'::jsonb,
actions jsonb not null
);
