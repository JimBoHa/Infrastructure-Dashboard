alter table nodes
    add column if not exists external_provider text;

alter table nodes
    add column if not exists external_id text;

create unique index if not exists nodes_external_identity_unique_idx
    on nodes (external_provider, external_id);

create index if not exists nodes_external_provider_idx
    on nodes (external_provider);
