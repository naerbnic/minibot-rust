use crate::util::table::{Index, Table, Error as TableError};


#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TableError(#[from] TableError),
}

pub type Result<T> = std::result::Result<T, Error>;

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

pub struct AccountService {
    table: Table<Account>,
    streamer_user_id_index: Index<Account, u64>,
    bot_user_id_index: Index<Account, u64>,
}

impl AccountService {
    pub fn new() -> Self {
        let mut table = Table::new();
        let streamer_user_id_index =
            table.add_index_borrowed(|a: &Account| &a.streamer_account.user_id);
        let bot_user_id_index = table.add_index_borrowed(|a| &a.bot_account.user_id);

        AccountService {
            table,
            streamer_user_id_index,
            bot_user_id_index,
        }
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
