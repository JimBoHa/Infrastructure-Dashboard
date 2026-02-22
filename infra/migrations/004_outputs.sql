create table if not exists outputs (
    id text primary key,
    node_id uuid not null references nodes(id) on delete cascade,
    name text not null,
    type text not null,
    state text not null default 'unknown',
    supported_states jsonb not null default '[]'::jsonb,
    last_command timestamptz,
    config jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);
create index if not exists outputs_node_idx on outputs(node_id);
