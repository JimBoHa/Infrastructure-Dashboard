alter table users
    add column if not exists password_hash text;
