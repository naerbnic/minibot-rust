use crate::services::base::token_store::TokenStoreHandle;

use async_trait::async_trait;
use fernet::Fernet;

use crate::services::base::token_store::TokenStore;

struct FernetTokenStore {
    encdec: Fernet,
}

impl FernetTokenStore {
    pub fn new() -> Self {
        FernetTokenStore {
            encdec: Fernet::new(&Fernet::generate_key()).unwrap(),
        }
    }
}

#[async_trait]
impl TokenStore for FernetTokenStore {
    async fn to_token(&self, value: &[u8]) -> anyhow::Result<String> {
        let encrypted = self.encdec.encrypt(value);
        Ok(encrypted)
    }

    async fn from_token(&self, token: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let decrypted = self
            .encdec
            .decrypt(token)
            .map_err(|_| anyhow::anyhow!("Unable to decrypt token."))?;
        Ok(Some(decrypted))
    }
}

pub fn create() -> TokenStoreHandle {
    TokenStoreHandle::new(FernetTokenStore::new())
}
