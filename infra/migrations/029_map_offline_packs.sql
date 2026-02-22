create table if not exists map_offline_packs (
    id text primary key,
    name text not null,
    bounds jsonb not null,
    min_zoom int not null,
    max_zoom int not null,
    status text not null default 'not_installed',
    progress jsonb not null default '{}'::jsonb,
    error text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists map_offline_packs_status_idx
    on map_offline_packs (status);

insert into map_offline_packs (id, name, bounds, min_zoom, max_zoom, status, created_at, updated_at)
values (
    'swanton_ca',
    'Swanton, CA (offline map pack)',
    jsonb_build_object(
        'min_lat', 36.95,
        'min_lng', -122.35,
        'max_lat', 37.15,
        'max_lng', -122.05
    ),
    10,
    18,
    'not_installed',
    now(),
    now()
)
on conflict (id) do nothing;
