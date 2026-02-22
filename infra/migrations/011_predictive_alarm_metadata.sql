-- Predictive alarm metadata
-- Extend alarms and alarm_events to capture origin and optional anomaly confidence.

alter table if exists alarms
    add column if not exists origin text not null default 'threshold',
    add column if not exists anomaly_score double precision;

update alarms set origin = 'threshold' where origin is null;

create index if not exists alarms_origin_idx on alarms(origin);

alter table if exists alarm_events
    add column if not exists origin text not null default 'threshold',
    add column if not exists anomaly_score double precision;

update alarm_events set origin = 'threshold' where origin is null;

create index if not exists alarm_events_origin_idx on alarm_events(origin);
