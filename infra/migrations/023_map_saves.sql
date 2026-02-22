create table if not exists map_saves (
    id bigserial primary key,
    name text not null,
    active_base_layer_id bigint references map_layers(id) on delete set null,
    center_lat double precision,
    center_lng double precision,
    zoom double precision,
    bearing double precision,
    pitch double precision,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

alter table map_settings
    add column if not exists active_save_id bigint references map_saves(id) on delete set null;

alter table map_features
    add column if not exists save_id bigint references map_saves(id) on delete cascade;

do $$
declare
    default_save_id bigint;
begin
    if not exists (select 1 from map_saves) then
        insert into map_saves (
            name,
            active_base_layer_id,
            center_lat,
            center_lng,
            zoom,
            bearing,
            pitch,
            created_at,
            updated_at
        )
        select
            'Default',
            ms.active_base_layer_id,
            ms.center_lat,
            ms.center_lng,
            ms.zoom,
            ms.bearing,
            ms.pitch,
            now(),
            now()
        from map_settings ms
        where ms.singleton = true
        returning id into default_save_id;

        if default_save_id is null then
            insert into map_saves (
                name,
                active_base_layer_id,
                center_lat,
                center_lng,
                zoom,
                bearing,
                pitch,
                created_at,
                updated_at
            )
            values ('Default', null, null, null, null, null, null, now(), now())
            returning id into default_save_id;
        end if;
    else
        select id into default_save_id from map_saves order by id asc limit 1;
    end if;

    insert into map_settings (singleton, active_save_id, created_at, updated_at)
    values (true, default_save_id, now(), now())
    on conflict (singleton)
    do update set
        active_save_id = coalesce(map_settings.active_save_id, excluded.active_save_id),
        updated_at = now();

    update map_features
    set save_id = (select active_save_id from map_settings where singleton = true)
    where save_id is null;
end $$;

alter table map_features
    alter column save_id set not null;

alter table map_features
    drop constraint if exists map_features_node_id_key;

alter table map_features
    drop constraint if exists map_features_sensor_id_key;

create unique index if not exists map_features_unique_node_per_save
    on map_features (save_id, node_id)
    where node_id is not null;

create unique index if not exists map_features_unique_sensor_per_save
    on map_features (save_id, sensor_id)
    where sensor_id is not null;

create index if not exists map_features_save_id_idx
    on map_features (save_id);

