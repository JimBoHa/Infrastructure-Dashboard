create table if not exists users (
    id uuid primary key default gen_random_uuid(),
    name text not null,
    email text not null unique,
    role text not null,
    capabilities jsonb not null default '[]'::jsonb,
    last_login timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists user_permission_audit (
    id bigserial primary key,
    user_id uuid not null references users(id) on delete cascade,
    actor text,
    changes jsonb not null,
    created_at timestamptz not null default now()
);

create index if not exists user_permission_audit_user_idx
    on user_permission_audit (user_id, created_at desc);
