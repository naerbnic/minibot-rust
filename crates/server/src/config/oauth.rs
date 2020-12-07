use serde::{Deserialize, Serialize};

/// Information about an OAuth2 Provider needed to perform the standard code
/// exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    token_endpoint: String,
    authz_endpoint: String,
    jwks_keys_url: String,
    api_endpoint: String,
}

impl ProviderInfo {
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
pub struct ClientInfo {
    client_id: String,
    client_secret: String,
    redirect_url: String,
}

impl ClientInfo {
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
pub struct Config {
    provider: ProviderInfo,
    client: ClientInfo,
}

impl Config {
    pub fn new(provider: ProviderInfo, client: ClientInfo) -> Self {
        Config { provider, client }
    }
    pub fn provider(&self) -> &ProviderInfo {
        &self.provider
    }
    pub fn client(&self) -> &ClientInfo {
        &self.client
    }
    pub fn api_endpoint(&self) -> url::Url {
        self.provider.api_endpoint.parse().unwrap()
    }
}
