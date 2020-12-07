CREATE TABLE twitch_accounts (
    id TEXT NOT NULL PRIMARY KEY
);

CREATE TABLE twitch_access_tokens (
    account_id TEXT NOT NULL PRIMARY KEY REFERENCES twitch_accounts(id),
    token TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE TABLE twitch_refresh_tokens (
    account_id TEXT NOT NULL PRIMARY KEY REFERENCES twitch_accounts(id),
    token TEXT NOT NULL
);

CREATE TABLE users (
    id INTEGER NOT NULL PRIMARY KEY ASC,
    twitch_id TEXT NOT NULL REFERENCES twitch_accounts(id) UNIQUE
);

CREATE TABLE user_bots (
    user_id TEXT NOT NULL PRIMARY KEY REFERENCES users(id),
    bot_account TEXT NOT NULL REFERENCES twitch_accounts(id)
);

CREATE TABLE minibot_tokens (
    id INTEGER NOT NULL PRIMARY KEY ASC,
    user_id INTEGER NOT NULL REFERENCES users(id),
    token TEXT NOT NULL
);