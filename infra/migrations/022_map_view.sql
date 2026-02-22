create table if not exists map_layers (
    id bigserial primary key,
    system_key text unique,
    name text not null,
    kind text not null default 'overlay',
    source_type text not null,
    config jsonb not null default '{}'::jsonb,
    opacity double precision not null default 1.0,
    enabled boolean not null default true,
    z_index int not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists map_layers_kind_idx
    on map_layers (kind);

create index if not exists map_layers_enabled_idx
    on map_layers (enabled);

create table if not exists map_settings (
    singleton boolean primary key default true,
    active_base_layer_id bigint references map_layers(id) on delete set null,
    center_lat double precision,
    center_lng double precision,
    zoom double precision,
    bearing double precision,
    pitch double precision,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists map_features (
    id bigserial primary key,
    node_id uuid unique references nodes(id) on delete set null,
    sensor_id varchar(24) unique references sensors(sensor_id) on delete set null,
    geometry jsonb not null,
    properties jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists map_features_created_idx
    on map_features (created_at desc);
