alter table if exists schedules
    add column if not exists blocks jsonb not null default '[]'::jsonb;
