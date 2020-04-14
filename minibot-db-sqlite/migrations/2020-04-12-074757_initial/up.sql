PRAGMA foreign_keys=ON;

CREATE TABLE twitch_refresh_tokens (
    id INTEGER NOT NULL PRIMARY KEY ASC,
    account_id INTEGER NOT NULL REFERENCES twitch_accounts(id),
    token TEXT NOT NULL
);

CREATE TABLE twitch_accounts (
    id INTEGER NOT NULL PRIMARY KEY ASC,
    twitch_id INTEGER NOT NULL,
    access_token TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE TABLE users (
    id INTEGER NOT NULL PRIMARY KEY ASC,
    streamer_account INTEGER NOT NULL REFERENCES twitch_accounts(id),
    bot_account INTEGER NOT NULL REFERENCES twitch_accounts(id)
);

CREATE TABLE minibot_tokens (
    id INTEGER NOT NULL PRIMARY KEY ASC,
    created_at INTEGER NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id),
    token TEXT NOT NULL
);