use crate::util::table::Table;

#[derive(Clone)]
pub struct TwitchAccount {
    user_id: u64,
    display_name: String,
    access_token: String,
    refresh_token: String,
}

#[derive(Clone)]
pub struct Account {
    streamer_account: TwitchAccount,
    bot_account: TwitchAccount,
}

pub struct AccountService(Table<Account>);

const STREAMER_USER_ID_INDEX: &str = "streamer_user_id";
const BOT_USER_ID_INDEX: &str = "bot_user_id";

impl AccountService {
    pub fn new() -> Self {
        AccountService(
            Table::<Account>::builder()
                .add_index_borrowed(STREAMER_USER_ID_INDEX, |a| &a.streamer_account.user_id)
                .add_index_borrowed(BOT_USER_ID_INDEX, |a| &a.bot_account.user_id)
                .build(),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn smoke_test() {
        let _ = AccountService::new();
    }
}


