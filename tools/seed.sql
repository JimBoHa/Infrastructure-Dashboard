-- Idempotent demo seed: 2 nodes, 5 sensors, ~1-min metrics for last 24h
create extension if not exists pgcrypto;

-- Nodes
insert into nodes (name, mac_eth, mac_wifi, status, last_seen)
values ('node-1','02:00:00:00:00:01','02:00:00:00:01:01','online', now())
on conflict (mac_eth, mac_wifi) do nothing;

insert into nodes (name, mac_eth, mac_wifi, status, last_seen)
values ('node-2','02:00:00:00:00:02','02:00:00:00:02:02','online', now())
on conflict (mac_eth, mac_wifi) do nothing;

-- Helper to ensure a sensor exists
with n as (select id from nodes where name='node-1' limit 1)
insert into sensors (sensor_id,node_id,name,type,unit,interval_seconds,rolling_avg_seconds)
select
substr(encode(digest('n1-temp'||clock_timestamp()::text,'sha256'),'hex'),1,24),
(select id from n),'temp_out','temperature','°C',900,900
where not exists (select 1 from sensors s join n on s.node_id=n.id where s.name='temp_out');

with n as (select id from nodes where name='node-1' limit 1)
insert into sensors (sensor_id,node_id,name,type,unit,interval_seconds,rolling_avg_seconds)
select substr(encode(digest('n1-moist'||clock_timestamp()::text,'sha256'),'hex'),1,24),
(select id from n),'moisture_a','moisture','%',900,900
where not exists (select 1 from sensors s join n on s.node_id=n.id where s.name='moisture_a');

with n as (select id from nodes where name='node-1' limit 1)
insert into sensors (sensor_id,node_id,name,type,unit,interval_seconds,rolling_avg_seconds)
select substr(encode(digest('n1-wind'||clock_timestamp()::text,'sha256'),'hex'),1,24),
(select id from n),'wind','wind','m/s',30,60
where not exists (select 1 from sensors s join n on s.node_id=n.id where s.name='wind');

with n as (select id from nodes where name='node-2' limit 1)
insert into sensors (sensor_id,node_id,name,type,unit,interval_seconds,rolling_avg_seconds)
select substr(encode(digest('n2-press'||clock_timestamp()::text,'sha256'),'hex'),1,24),
(select id from n),'pressure','pressure','kPa',30,60
where not exists (select 1 from sensors s join n on s.node_id=n.id where s.name='pressure');

with n as (select id from nodes where name='node-2' limit 1)
insert into sensors (sensor_id,node_id,name,type,unit,interval_seconds,rolling_avg_seconds)
select substr(encode(digest('n2-power'||clock_timestamp()::text,'sha256'),'hex'),1,24),
(select id from n),'power','power','W',1,60
where not exists (select 1 from sensors s join n on s.node_id=n.id where s.name='power');

-- Clean existing last-24h metrics for these sensors (idempotency)
do $$
declare
s record;
begin
for s in
select sensor_id,name from sensors
where name in ('temp_out','moisture_a','wind','pressure','power')
loop
delete from metrics where sensor_id=s.sensor_id and ts >= now() - interval '24 hour';
end loop;
end$$;

-- Insert ~1-min samples for last 24h per sensor (synthetic waveforms)
insert into metrics (sensor_id, ts, value, quality)
select s.sensor_id,
t.ts,
case s.name
when 'temp_out'  then 18.0 + 7.5 * sin(extract(epoch from t.ts)/3600.0) + random()*0.5
when 'moisture_a'then 42.0 + 3.0 * sin(extract(epoch from t.ts)/18000.0) + random()*0.3
when 'wind'      then  3.0 + 2.0 * abs(sin(extract(epoch from t.ts)/300.0)) + random()*0.5
when 'pressure'  then 98.0 + 1.0 * sin(extract(epoch from t.ts)/7200.0) + random()*0.2
when 'power'     then 500.0 + 150.0 * sin(extract(epoch from t.ts)/600.0) + random()*10.0
else random()*10.0
end,
0
from sensors s
cross join lateral (
select generate_series(date_trunc('minute', now() - interval '24 hour'),
date_trunc('minute', now()),
interval '1 minute') as ts
) t
where s.name in ('temp_out','moisture_a','wind','pressure','power');
