use crate::util::table::{Index, Table, Uniqueness};

use crate::services::base::account::{Account, AccountService, Result};
use crate::util::error::ResultExt as _;

pub struct InMemoryAccountService {
    table: Table<Account>,
    streamer_user_id_index: Index<Account, u64>,
    bot_user_id_index: Index<Account, u64>,
}

impl InMemoryAccountService {
    pub fn new() -> Result<Self> {
        let mut table = Table::new();
        let streamer_user_id_index = table
            .add_index_borrowed(Uniqueness::NotUnique, |a: &Account| {
                &a.streamer_account.user_id
            })
            .map_err_internal()?;
        let bot_user_id_index = table
            .add_index_borrowed(Uniqueness::NotUnique, |a| &a.bot_account.user_id)
            .map_err_internal()?;

        Ok(InMemoryAccountService {
            table,
            streamer_user_id_index,
            bot_user_id_index,
        })
    }
}

#[async_trait::async_trait]
impl AccountService for InMemoryAccountService {
    async fn create_account(&self, acct: Account) -> Result<u64> {
        Ok(self.table.add(acct).map_err_internal()?)
    }

    async fn get_account(&self, user_id: u64) -> Result<Option<Account>> {
        Ok(self.table.get(user_id).map_err_internal()?)
    }
}
