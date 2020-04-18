table! {
    minibot_tokens (id) {
        id -> BigInt,
        created_at -> BigInt,
        user_id -> BigInt,
        token -> Text,
    }
}

table! {
    twitch_access_tokens (account_id) {
        account_id -> Text,
        token -> Text,
        expires_at -> BigInt,
    }
}

table! {
    twitch_accounts (id) {
        id -> Text,
    }
}

table! {
    twitch_logins (twitch_id) {
        twitch_id -> Text,
        user_id -> BigInt,
    }
}

table! {
    twitch_refresh_tokens (account_id) {
        account_id -> Text,
        token -> Text,
    }
}

table! {
    user_bots (user_id) {
        user_id -> BigInt,
        bot_account -> Text,
    }
}

table! {
    users (id) {
        id -> BigInt,
        twitch_id -> Text,
    }
}

joinable!(minibot_tokens -> users (user_id));
joinable!(twitch_access_tokens -> twitch_accounts (account_id));
joinable!(twitch_logins -> twitch_accounts (twitch_id));
joinable!(twitch_logins -> users (user_id));
joinable!(twitch_refresh_tokens -> twitch_accounts (account_id));
joinable!(user_bots -> twitch_accounts (bot_account));

allow_tables_to_appear_in_same_query!(
    minibot_tokens,
    twitch_access_tokens,
    twitch_accounts,
    twitch_logins,
    twitch_refresh_tokens,
    user_bots,
    users,
);
