-- Restore history for backup bundles.
-- Used by /api/restore and /api/restores/recent.

create table if not exists restore_events (
    id uuid primary key default gen_random_uuid(),
    backup_node_id uuid not null,
    backup_date date not null,
    target_node_id uuid not null,
    status text not null check (status in ('queued', 'running', 'ok', 'error')),
    message text,
    actor_user_id uuid,
    actor_email text,
    attempt_count int not null default 0,
    last_attempt_at timestamptz,
    started_at timestamptz,
    finished_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists restore_events_status_idx
    on restore_events (status, created_at desc);

create index if not exists restore_events_created_idx
    on restore_events (created_at desc);
