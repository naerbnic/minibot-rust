use minibot_common::proof_key::Challenge;
use serde::{Deserialize, Serialize};

pub mod endpoints;
pub mod handlers;
pub mod middleware;

/// Info stored between the post to the minibot auth exchange start and the
/// OAuth2 redirect response.
#[derive(Clone, Serialize, Deserialize)]
pub struct AuthRequestInfo {
    /// The local redirect URL provided by a user.
    pub local_redirect: String,

    /// The challenge string provided by a user.
    pub challenge: Challenge,
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
    pub challenge: Challenge,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdentityInfo {
    twitch_id: String,
    twitch_auth_token: String,
}
