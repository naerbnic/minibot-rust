
table! {
    minibot_tokens (id) {
        id -> BigInt,
        created_at -> BigInt,
        user_id -> BigInt,
        token -> Text,
    }
}

table! {
    twitch_accounts (id) {
        id -> BigInt,
        twitch_id -> BigInt,
        access_token -> Text,
        expires_at -> BigInt,
    }
}

table! {
    twitch_refresh_tokens (id) {
        id -> BigInt,
        account_id -> BigInt,
        token -> Text,
    }
}

table! {
    users (id) {
        id -> BigInt,
        streamer_account -> BigInt,
        bot_account -> BigInt,
    }
}

joinable!(minibot_tokens -> users (user_id));
joinable!(twitch_refresh_tokens -> twitch_accounts (account_id));

allow_tables_to_appear_in_same_query!(
    minibot_tokens,
    twitch_accounts,
    twitch_refresh_tokens,
    users,
);
