use async_trait::async_trait;
use std::sync::Arc;

/// A service that stores/converts `AuthRequestInfo` to and from a string token.
#[async_trait]
pub trait TokenService<T>: Sync + Send {
    /// Return a token for the given info value. This token must be a url-safe
    /// string. `self.from_token()` must return the same value.
    async fn to_token(&self, value: T) -> anyhow::Result<String>;

    /// Return a value of type T for a given token.
    ///
    /// A real implementation must ensure that the token has not been modified
    /// externally, or return an error otherwise.
    async fn from_token(&self, token: &str) -> anyhow::Result<Option<T>>;
}

#[derive(gotham_derive::StateData)]
pub struct TokenServiceHandle<T: 'static>(
    Arc<dyn TokenService<T> + Send + Sync + std::panic::RefUnwindSafe>,
);

impl<T: 'static> TokenServiceHandle<T> {
    pub fn new<S: TokenService<T> + Send + Sync + std::panic::RefUnwindSafe + 'static>(
        token_svc: S,
    ) -> Self {
        TokenServiceHandle(Arc::new(token_svc))
    }
}

impl<T: 'static> Clone for TokenServiceHandle<T> {
    fn clone(&self) -> Self {
        TokenServiceHandle(self.0.clone())
    }
}

impl<T> std::ops::Deref for TokenServiceHandle<T> {
    type Target = dyn TokenService<T> + 'static;
    fn deref(&self) -> &(dyn TokenService<T> + 'static) {
        &*self.0
    }
}
