use async_trait::async_trait;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Info stored between the post to the minibot auth exchange start and the
/// OAuth2 redirect response.
#[derive(Clone, Serialize, Deserialize)]
pub struct AuthRequestInfo {
    /// The local redirect URL provided by a user.
    pub local_redirect: String,

    /// The challenge string provided by a user.
    pub challenge: String,
}

/// A service that stores/converts `AuthRequestInfo` to and from a string token.
#[async_trait]
pub trait TokenService<T>: Sync {
    /// Return a token for the given info value. This token must be a url-safe
    /// string. `self.from_token()` must return the same value.
    async fn to_token(&self, value: T) -> Result<String, anyhow::Error>;

    /// Return a value of type T for a given token.
    /// 
    /// A real implementation must ensure that the token has not been modified
    /// externally, or return an error otherwise.
    async fn from_token(&self, token: &str) -> Result<T, anyhow::Error>;
}

pub type AuthService = dyn TokenService<AuthRequestInfo>;

/// Info stored between returning the token via redirect to the user and the
/// user submitting the token to the account-create/bot-add endpoint with the
/// challenge verifier
#[derive(Clone, Serialize, Deserialize)]
pub struct AuthConfirmInfo {
    /// The code returned by the OAuth2 provider that can be exchanged for a
    /// token.
    pub code: String,

    /// The challenge provided by the user. By providing a verifier, it
    /// ensures that the final use of the token on the endpoint is from the
    /// person who requested it.
    pub challenge: String,
}

pub type AuthConfirmService = dyn TokenService<AuthConfirmInfo> + Sync;

#[derive(Clone, Serialize, Deserialize)]
pub struct IdentityInfo {
    twitch_id: String,
    twitch_auth_token: String
}

pub struct SerdeTokenService<T> where T: Serialize + DeserializeOwned + Sync + Send {
    data: std::marker::PhantomData<T>,
}

#[async_trait]
impl<T: Serialize + DeserializeOwned + Sync + Send> TokenService<T> for SerdeTokenService<T> {
    async fn to_token(&self, value: T) -> Result<String, anyhow::Error> {
        Ok(serde_json::to_string(&value)?)
    }
    async fn from_token(&self, token: &str) -> Result<T, anyhow::Error> {
        Ok(serde_json::from_str(token)?)
    }
}