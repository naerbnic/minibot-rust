CREATE TABLE ephemeral_tokens (
    token TEXT NOT NULL PRIMARY KEY,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    contents BYTEA NOT NULL
);

CREATE TABLE twitch_accounts (
    id TEXT NOT NULL PRIMARY KEY
);

CREATE TABLE twitch_access_tokens (
    account_id TEXT NOT NULL PRIMARY KEY REFERENCES twitch_accounts(id),
    token TEXT NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE
);

CREATE TABLE twitch_refresh_tokens (
    account_id TEXT NOT NULL PRIMARY KEY REFERENCES twitch_accounts(id),
    token TEXT NOT NULL
);

CREATE TABLE users (
    id SERIAL NOT NULL PRIMARY KEY,
    twitch_id TEXT NOT NULL REFERENCES twitch_accounts(id) UNIQUE
);

CREATE TABLE user_bots (
    user_id INTEGER NOT NULL PRIMARY KEY REFERENCES users(id),
    bot_account TEXT NOT NULL REFERENCES twitch_accounts(id)
);

CREATE TABLE minibot_tokens (
    id SERIAL NOT NULL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    token TEXT NOT NULL
);

CREATE INDEX minibot_tokens_by_users ON minibot_tokens (user_id);