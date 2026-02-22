create table if not exists alarm_events (
    id bigserial primary key,
    alarm_id bigint references alarms(id) on delete set null,
    sensor_id char(24),
    node_id uuid,
    status text not null,
    message text,
    created_at timestamptz not null default now()
);
create index if not exists alarm_events_sensor_idx on alarm_events(sensor_id);
create index if not exists alarm_events_node_idx on alarm_events(node_id);
create index if not exists alarm_events_created_idx on alarm_events(created_at);

create table if not exists action_logs (
    id bigserial primary key,
    schedule_id bigint references schedules(id) on delete cascade,
    action jsonb not null,
    status text not null,
    message text,
    created_at timestamptz not null default now()
);
create index if not exists action_logs_schedule_idx on action_logs(schedule_id);
