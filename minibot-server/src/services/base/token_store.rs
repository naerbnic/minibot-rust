use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
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

    pub async fn to_token<T: TokenData>(&self, value: &T) -> anyhow::Result<String> {
        self.0
            .to_token(&serde_json::to_vec(&EncodedType {
                token_type: T::type_id().to_string(),
                val: value,
            })?)
            .await
    }

    pub async fn from_token<T: TokenData>(&self, token: &str) -> anyhow::Result<Option<T>> {
        if let Some(vec) = self.0.from_token(token).await? {
            let encoded_val: EncodedType<T> = serde_json::from_slice(&vec)?;
            anyhow::ensure!(
                encoded_val.token_type == T::type_id(),
                "Wrong token type. Got {:?}, expected {:?}",
                encoded_val.token_type,
                T::type_id()
            );

            Ok(Some(encoded_val.val))
        } else {
            Ok(None)
        }
    }
}

impl std::fmt::Debug for TokenStoreHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenStoreHandle()")
    }
}

pub trait TokenData: Serialize + DeserializeOwned + 'static {
    fn type_id() -> &'static str {
        // Note: This should not have a default implementation like this, as type names may change,
        // and must be stable. Remove before final version.
        std::any::type_name::<Self>()
    }
}

#[derive(Serialize, Deserialize)]
struct EncodedType<T> {
    #[serde(rename = "type")]
    token_type: String,
    val: T,
}

impl<T: TokenData> EncodedType<T> {}

#[derive(Clone, Debug)]
pub struct TypedTokenStore<T> {
    store: TokenStoreHandle,
    _phantom: std::marker::PhantomData<fn() -> T>,
}

impl<T: TokenData> TypedTokenStore<T> {
    pub async fn to_token(&self, value: &T) -> anyhow::Result<String> {
        self.store.0.to_token(&serde_json::to_vec(value)?).await
    }

    pub async fn from_token(&self, token: &str) -> anyhow::Result<Option<T>> {
        match self.store.0.from_token(token).await {
            Ok(Some(vec)) => Ok(Some(serde_json::from_slice(&vec)?)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
