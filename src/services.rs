use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
pub trait AuthService: Sync {
    /// Return a token for the given request info. This token must be a url-safe
    /// string. `self.token_to_request()` must return the same AuthRequestInfo
    /// value.
    async fn request_to_token(&self, req: AuthRequestInfo) -> Result<String, anyhow::Error>;

    /// Return an AuthRequestInfo value for a given token.
    /// 
    /// A real implementation must ensure that the token has not been modified
    /// externally, or return an error otherwise.
    async fn token_to_request(&self, token: &str) -> Result<AuthRequestInfo, anyhow::Error>;
}

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

#[async_trait]
pub trait AuthConfirmService: Sync {
    async fn confirm_to_token(&self, req: AuthConfirmInfo) -> Result<String, anyhow::Error>;
    async fn token_to_confirm(&self, token: &str) -> Result<AuthConfirmInfo, anyhow::Error>;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdentityInfo {
    twitch_id: String,
    twitch_auth_token: String
}
