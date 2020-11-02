use serde::{Deserialize, Serialize};

/// Information about an OAuth2 Provider needed to perform the standard code
/// exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderInfo {
    token_endpoint: String,
    authz_endpoint: String,
    jwks_keys_url: String,
    api_endpoint: String,
}

impl OAuthProviderInfo {
    /// The URL for the token exchange endpoint.
    pub fn token_endpoint(&self) -> &str {
        &self.token_endpoint
    }
    /// The URL for the authorization endpoint.
    pub fn authz_endpoint(&self) -> &str {
        &self.authz_endpoint
    }
    /// The URL for the JSON Web Token keys used to verify OpenID identity
    /// tokens.
    pub fn jwks_keys_url(&self) -> &str {
        &self.jwks_keys_url
    }
    pub fn api_endpoint(&self) -> &str {
        &self.api_endpoint
    }
}

/// Information about an OAuth2 Client/App needed to perform the standard code
/// exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClientInfo {
    client_id: String,
    client_secret: String,
    redirect_url: String,
}

impl OAuthClientInfo {
    /// The client ID string associated with the application.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    /// The client secret string associated with the application.
    pub fn client_secret(&self) -> &str {
        &self.client_secret
    }
    /// The redirect URL assigned to the client.
    pub fn redirect_url(&self) -> &str {
        &self.redirect_url
    }
}

/// All information about the OAuth2 environment needed to perform the standard
/// code exchange.
#[derive(Debug, Clone, Serialize, Deserialize, gotham_derive::StateData)]
pub struct OAuthConfig {
    provider: OAuthProviderInfo,
    client: OAuthClientInfo,
}

impl OAuthConfig {
    pub fn new(provider: OAuthProviderInfo, client: OAuthClientInfo) -> Self {
        OAuthConfig { provider, client }
    }
    pub fn provider(&self) -> &OAuthProviderInfo {
        &self.provider
    }
    pub fn client(&self) -> &OAuthClientInfo {
        &self.client
    }
    pub fn api_endpoint(&self) -> url::Url {
        self.provider.api_endpoint.parse().unwrap()
    }
}

lazy_static::lazy_static! {
    pub static ref TWITCH_PROVIDER: OAuthProviderInfo =
        serde_json::from_str(std::include_str!("twitch-provider.json")).unwrap();
}
