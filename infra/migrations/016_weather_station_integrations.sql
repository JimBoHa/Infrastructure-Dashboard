create table if not exists weather_station_integrations (
id uuid primary key default gen_random_uuid(),
node_id uuid not null references nodes(id) on delete cascade,
kind text not null default 'ws-2902',
nickname text not null,
protocol text not null,
token_hash text not null,
enabled boolean not null default true,
created_at timestamptz not null default now(),
rotated_at timestamptz,
last_seen timestamptz,
last_missing_fields jsonb not null default '[]'::jsonb,
last_payload jsonb not null default '{}'::jsonb
);

create unique index if not exists weather_station_integrations_token_hash_uq
on weather_station_integrations(token_hash);

create index if not exists weather_station_integrations_node_id_idx
on weather_station_integrations(node_id);
