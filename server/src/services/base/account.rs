use crate::util::error::FromInternalError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Internal error: {0:?}")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl FromInternalError for Error {
    fn from_internal<E: std::error::Error + Send + Sync + 'static>(err: E) -> Self {
        Error::Internal(Box::new(err))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct TwitchAccount {
    pub user_id: u64,
    pub display_name: String,
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Clone)]
pub struct Account {
    pub streamer_account: TwitchAccount,
    pub bot_account: TwitchAccount,
}

#[async_trait::async_trait]
pub trait AccountStore: Send + Sync {
    async fn create_account(&self, acct: Account) -> Result<u64>;
    async fn get_account(&self, user_id: u64) -> Result<Option<Account>>;
}

define_deref_handle!(AccountStoreHandle, AccountStore);