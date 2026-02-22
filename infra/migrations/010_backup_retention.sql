create table if not exists backup_retention (
    node_id uuid primary key references nodes(id) on delete cascade,
    keep_days int not null check (keep_days > 0),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
