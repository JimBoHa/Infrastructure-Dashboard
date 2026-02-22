create table if not exists adoption_tokens (
token varchar(64) primary key,
mac_eth macaddr,
mac_wifi macaddr,
service_name text,
metadata jsonb not null default '{}'::jsonb,
expires_at timestamptz not null,
created_at timestamptz not null default now(),
used_at timestamptz,
node_id uuid references nodes(id) on delete set null
);
