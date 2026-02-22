create table if not exists api_tokens (
id uuid primary key default gen_random_uuid(),
name text,
token_hash text not null,
capabilities jsonb not null default '[]'::jsonb,
created_at timestamptz not null default now(),
last_used_at timestamptz,
expires_at timestamptz,
revoked_at timestamptz
);

create unique index if not exists api_tokens_token_hash_uq
on api_tokens(token_hash);

create index if not exists api_tokens_revoked_at_idx
on api_tokens(revoked_at);

create index if not exists api_tokens_expires_at_idx
on api_tokens(expires_at);
