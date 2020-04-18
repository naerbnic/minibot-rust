mod minibot_tokens {
    use crate::schema::minibot_tokens;

    #[derive(Queryable, Debug)]
    pub struct MinibotToken {
        pub id: i64,
        pub user_id: i64,
        pub token: String,
    }

    #[derive(Insertable, Debug)]
    #[table_name = "minibot_tokens"]
    pub struct NewMinibotToken<'a> {
        pub user_id: i64,
        pub token: &'a str,
    }
}

mod twitch_accounts {
    use crate::schema::twitch_accounts;

    #[derive(Queryable, Debug)]
    pub struct TwitchAccount {
        pub id: String,
    }

    #[derive(Insertable, Debug)]
    #[table_name = "twitch_accounts"]
    pub struct NewTwitchAccount<'a> {
        pub id: &'a str,
    }
}

mod twitch_refresh_tokens {
    use crate::schema::twitch_refresh_tokens;

    #[derive(Queryable, Debug)]
    pub struct TwitchRefreshToken {
        pub account_id: String,
        pub token: String,
    }

    #[derive(Insertable, Debug)]
    #[table_name = "twitch_refresh_tokens"]
    pub struct NewTwitchRefreshToken<'a> {
        pub account_id: &'a str,
        pub token: &'a str,
    }
}

mod twitch_access_tokens {
    use crate::schema::twitch_access_tokens;

    #[derive(Queryable, Debug)]
    pub struct TwitchAccessToken {
        pub account_id: String,
        pub token: String,
        pub expires_at: i64,
    }

    #[derive(Insertable, Debug)]
    #[table_name = "twitch_access_tokens"]
    pub struct NewTwitchAccessToken<'a> {
        pub account_id: &'a str,
        pub token: &'a str,
        pub expires_at: i64,
    }
}

mod users {
    use crate::schema::users;

    #[derive(Queryable, Debug)]
    pub struct User {
        pub id: i64,
        pub twitch_id: String,
    }

    #[derive(Insertable, Debug)]
    #[table_name = "users"]
    pub struct NewUser<'a> {
        pub twitch_id: &'a str,
    }
}

mod user_bots {
    use crate::schema::user_bots;

    #[derive(Queryable, Debug)]
    pub struct UserBot {
        pub user_id: i64,
        pub bot_account: String,
    }

    #[derive(Insertable, Debug)]
    #[table_name = "user_bots"]
    pub struct NewUserBot<'a> {
        pub user_id: i64,
        pub bot_account: &'a str,
    }
}

pub use self::{
    minibot_tokens::{MinibotToken, NewMinibotToken},
    twitch_access_tokens::{NewTwitchAccessToken, TwitchAccessToken},
    twitch_accounts::{NewTwitchAccount, TwitchAccount},
    twitch_refresh_tokens::{NewTwitchRefreshToken, TwitchRefreshToken},
    user_bots::{NewUserBot, UserBot},
    users::{NewUser, User},
};
