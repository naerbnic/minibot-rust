create extension pgcrypto;

-- Ephemeral tokens. These are used for arbitrary short-lived data.
-- It should not be referenced
create table ephemeral_tokens (
    token bytea primary key,
    created_at timestamp with time zone not null,
    expires_at timestamp with time zone not null,
    contents bytea not null
);
create index on ephemeral_tokens (expires_at);

create function create_ephemeral_token(
    contents bytea, 
    created_at timestamp with time zone,
    expires_at timestamp with time zone)
returns text
as $$
declare token_bytes bytea;
declare token_string text;
begin
    select gen_random_bytes(15) into token_bytes;
    insert into ephemeral_tokens (
        token,
        created_at,
        expires_at,
        contents)
    values (
        token_bytes,
        created_at,
        expires_at,
        contents);
    token_string := encode(token_bytes, 'base64');
    token_string := replace(token_string, '/', '_');
    token_string := replace(token_string, '+', '-');
    return token_string;
end;
$$
language plpgsql;

create function get_ephemeral_token(
    token text)
returns table (
    contents bytea,
    expires_at timestamp with time zone)
as $$
declare token_bytes bytea;
begin
    token := replace(token, '_', '/');
    token := replace(token, '-', '+');
    token_bytes := decode(token, 'base64');
    return query
        select e.contents, e.expires_at
        from ephemeral_tokens as e
        where
            token_bytes = e.token
        limit 1;
end;
$$
language plpgsql;

create procedure clear_expired_ephemeral_tokens(horizon timestamptz)
as $$
begin
    delete from ephemeral_tokens as e
    where e.expires_at < horizon;
end;
$$
language plpgsql;

-- Twitch accounts and tokens

-- Base accounts entities. Represents any account used in the system.
create table twitch_accounts (
    id TEXT NOT NULL PRIMARY KEY
);

-- Concrete twitch access tokens used with OAuth interface.
create table twitch_access_tokens (
    id serial primary key,
    token text not null,
    created_at timestamptz not null,
    expires_at timestamptz not null
);
create index on twitch_access_tokens (expires_at);

create table twitch_scopes(
    scope text primary key
);

create table twitch_access_token_scopes (
    token_id integer not null references twitch_access_tokens (id) unique,
    scope text not null references twitch_scopes(scope),
    primary key (token_id, scope)
);

create table twitch_refresh_tokens (
    id serial primary key,
    token text not null
);

create table twitch_refresh_token_scopes (
    token_id integer not null references twitch_refresh_tokens (id) unique,
    scope text not null references twitch_scopes(scope),
    primary key (token_id, scope)
);

create table users (
    id serial primary key,
    user_name text not null unique
);

create table openid_provider(
    provider_name text primary key,
    base_url text not null
);

create table openid_identity(
    id serial primary key,
    sub text not null
);

create table user_openid_identity(
    user_id integer references users(id),
    identity_id integer references openid_identity(id) unique,
    primary key (user_id, identity_id)
);

create table openid_identity_provider(
    identity_id integer references openid_identity(id) unique,
    provider_name text references openid_provider(provider_name),
    primary key (identity_id, provider_name)
);

create table user_bots (
    user_id integer references users(id) unique,
    bot_account text not null references twitch_accounts(id),
    primary key (user_id, bot_account)
);

create table persistent_tokens (
    id serial primary key,
    token text not null unique,
    created_at timestamptz not null,
    last_used_at timestamptz not null,
    expires_at timestamptz not null
);

-- Relation mapping persistent tokens to their user.
create table user_persistent_tokens (
    user_id integer references users(id),
    token_id integer references persistent_tokens(id)
);