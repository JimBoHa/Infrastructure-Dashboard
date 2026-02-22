-- Incidents v1: group alarm events into operator-facing incidents with notes.

create table if not exists incidents (
    id bigserial primary key,
    rule_id bigint references alarm_rules(id) on delete set null,
    target_key text,
    severity text not null default 'warning',
    status text not null default 'open',
    title text not null default '',
    assigned_to uuid references users(id) on delete set null,
    snoozed_until timestamptz,
    first_event_at timestamptz not null,
    last_event_at timestamptz not null,
    closed_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists incidents_status_last_event_idx on incidents(status, last_event_at desc);
create index if not exists incidents_rule_target_last_event_idx on incidents(rule_id, target_key, last_event_at desc);
create index if not exists incidents_assigned_status_idx on incidents(assigned_to, status);

create table if not exists incident_notes (
    id bigserial primary key,
    incident_id bigint not null references incidents(id) on delete cascade,
    created_by uuid references users(id) on delete set null,
    body text not null,
    created_at timestamptz not null default now()
);

create index if not exists incident_notes_incident_created_idx on incident_notes(incident_id, created_at desc);

alter table if exists alarm_events
    add column if not exists incident_id bigint references incidents(id) on delete set null,
    add column if not exists target_key text;

create index if not exists alarm_events_incident_idx on alarm_events(incident_id);
create index if not exists alarm_events_rule_target_created_idx on alarm_events(rule_id, target_key, created_at desc);
create index if not exists alarm_events_target_key_created_idx on alarm_events(target_key, created_at desc);

