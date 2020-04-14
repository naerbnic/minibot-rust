mod minibot_tokens {
    use crate::schema::minibot_tokens;

    #[derive(Queryable, Debug)]
    pub struct MinibotToken {
        id: i64,
        created_at: i64,
        user_id: i64,
        token: String,
    }

    #[derive(Insertable, Debug)]
    #[table_name="minibot_tokens"]
    pub struct NewMinibotToken<'a> {
        created_at: i64,
        user_id: i64,
        token: &'a str,
    }
}

mod twitch_accounts {
    use crate::schema::twitch_accounts;

    #[derive(Queryable, Debug)]
    pub struct TwitchAccount {
        id: i64,
        twitch_id: i64,
        access_token: String,
        expires_at: i64,
    }

    #[derive(Insertable, Debug)]
    #[table_name="twitch_accounts"]
    pub struct NewTwitchAccount<'a> {
        twitch_id: i64,
        access_token: &'a str,
        expires_at: i64,
    }
}

mod twitch_refresh_tokens {
    use crate::schema::twitch_refresh_tokens;

    #[derive(Queryable, Debug)]
    pub struct TwitchRefreshToken {
        id: i64,
        account_id: i64,
        token: String,
    }

    #[derive(Insertable, Debug)]
    #[table_name="twitch_refresh_tokens"]
    pub struct NewTwitchRefreshToken<'a> {
        account_id: i64,
        token: &'a str,
    }
}

mod users {
    use crate::schema::users;

    #[derive(Queryable, Debug)]
    pub struct User {
        id: i64,
        account_id: i64,
        token: String,
    }

    #[derive(Insertable, Debug)]
    #[table_name="users"]
    pub struct NewUser {
        streamer_account: i64,
        bot_account: i64,
    }
}

pub use self::{
    minibot_tokens::{MinibotToken, NewMinibotToken},
    twitch_accounts::{TwitchAccount, NewTwitchAccount},
    twitch_refresh_tokens::{TwitchRefreshToken, NewTwitchRefreshToken},
    users::{User, NewUser},
};