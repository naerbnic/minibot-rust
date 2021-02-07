create extension pgcrypto;

-- Ephemeral tokens. These all must have a token
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

create table twitch_accounts (
    id TEXT NOT NULL PRIMARY KEY
);

create table twitch_access_tokens (
    account_id TEXT NOT NULL PRIMARY KEY REFERENCES twitch_accounts(id),
    token TEXT NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE
);

create table twitch_refresh_tokens (
    account_id TEXT NOT NULL PRIMARY KEY REFERENCES twitch_accounts(id),
    token TEXT NOT NULL
);

create table users (
    id SERIAL NOT NULL PRIMARY KEY,
    twitch_id TEXT NOT NULL REFERENCES twitch_accounts(id) UNIQUE
);

create table user_bots (
    user_id INTEGER NOT NULL PRIMARY KEY REFERENCES users(id),
    bot_account TEXT NOT NULL REFERENCES twitch_accounts(id)
);

create table minibot_tokens (
    id SERIAL NOT NULL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    token TEXT NOT NULL
);

create index minibot_tokens_by_users ON minibot_tokens (user_id);