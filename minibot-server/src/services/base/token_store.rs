use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

/// A service that stores/converts `AuthRequestInfo` to and from a string token.
#[async_trait]
pub trait TokenStore: Sync + Send {
    /// Return a token for the given info value. This token must be a url-safe
    /// string. `self.from_token()` must return the same value.
    async fn to_token(&self, value: &[u8]) -> anyhow::Result<String>;

    /// Return a value of type T for a given token.
    ///
    /// A real implementation must ensure that the token has not been modified
    /// externally, or return an error otherwise.
    async fn from_token(&self, token: &str) -> anyhow::Result<Option<Vec<u8>>>;
}

#[derive(Clone, gotham_derive::StateData)]
pub struct TokenStoreHandle(Arc<dyn TokenStore + Send + Sync + std::panic::RefUnwindSafe>);

impl TokenStoreHandle {
    pub fn new<S: TokenStore + Send + Sync + std::panic::RefUnwindSafe + 'static>(
        token_svc: S,
    ) -> Self {
        TokenStoreHandle(Arc::new(token_svc))
    }

    pub async fn bytes_to_token(&self, bytes: &[u8]) -> anyhow::Result<String> {
        self.0.to_token(bytes).await
    }

    pub async fn bytes_from_token(&self, token: &str) -> anyhow::Result<Option<Vec<u8>>> {
        self.0.from_token(token).await
    }

    pub async fn val_to_token<T: Serialize>(&self, value: &T) -> anyhow::Result<String> {
        self.0.to_token(&serde_json::to_vec(value)?).await
    }

    pub async fn val_from_token<T: DeserializeOwned>(
        &self,
        token: &str,
    ) -> anyhow::Result<Option<T>> {
        match self.0.from_token(token).await {
            Ok(Some(vec)) => Ok(Some(serde_json::from_slice(&vec)?)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
