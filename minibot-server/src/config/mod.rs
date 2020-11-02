use serde::{Serialize, Deserialize};

/// Information about an OAuth2 Provider needed to perform the standard code
/// exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderInfo {
    /// The URL for the token exchange endpoint.
    pub token_endpoint: String,
    /// The URL for the authorization endpoing.
    pub authz_endpoint: String,
    /// The URL for the JSON Web Token keys used to verify OpenID identity
    /// tokens.
    pub jwks_keys_url: String,
    pub api_endpoint: String,
}

/// Information about an OAuth2 Client/App needed to perform the standard code
/// exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClientInfo {
    /// The client ID string associated with the application.
    pub client_id: String,
    /// The client secret string associated with the application.
    pub client_secret: String,
    /// The redirect URL assigned to the client.
    pub redirect_url: String,
}

/// All information about the OAuth2 environment needed to perform the standard
/// code exchange.
#[derive(Debug, Clone, Serialize, Deserialize, gotham_derive::StateData)]
pub struct OAuthConfig {
    pub provider: OAuthProviderInfo,
    pub client: OAuthClientInfo,
}

impl OAuthConfig {
    pub fn api_endpoint(&self) -> url::Url {
        self.provider.api_endpoint.parse().unwrap()
    }
}

lazy_static::lazy_static! {
    pub static ref TWITCH_PROVIDER: OAuthProviderInfo =
        serde_json::from_str(std::include_str!("twitch-provider.json")).unwrap();
}