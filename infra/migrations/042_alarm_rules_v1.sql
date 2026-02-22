-- Alarm Rules v1: separate user-authored alarm rule config from runtime alarm/event state.

create table if not exists alarm_rules (
    id bigserial primary key,
    name text not null,
    description text not null default '',
    enabled boolean not null default true,
    severity text not null default 'warning',
    origin text not null default 'threshold',
    target_selector jsonb not null default '{}'::jsonb,
    condition_ast jsonb not null default '{}'::jsonb,
    timing jsonb not null default '{}'::jsonb,
    message_template text not null default '',
    created_by uuid references users(id) on delete set null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    deleted_at timestamptz
);

create index if not exists alarm_rules_enabled_idx on alarm_rules(enabled, deleted_at);
create index if not exists alarm_rules_origin_idx on alarm_rules(origin);

create table if not exists alarm_rule_state (
    rule_id bigint not null references alarm_rules(id) on delete cascade,
    target_key text not null,
    currently_firing boolean not null default false,
    consecutive_hits integer not null default 0,
    window_state jsonb not null default '{}'::jsonb,
    last_eval_at timestamptz,
    last_value double precision,
    last_transition_at timestamptz,
    error text,
    primary key (rule_id, target_key)
);

create index if not exists alarm_rule_state_rule_idx on alarm_rule_state(rule_id);

alter table if exists alarms
    add column if not exists rule_id bigint references alarm_rules(id) on delete set null,
    add column if not exists target_key text,
    add column if not exists resolved_at timestamptz;

create index if not exists alarms_rule_target_idx on alarms(rule_id, target_key);

alter table if exists alarm_events
    add column if not exists rule_id bigint references alarm_rules(id) on delete set null,
    add column if not exists transition text;

