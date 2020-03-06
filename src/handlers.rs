use crate::services::{AuthConfirmInfo, AuthConfirmService, AuthRequestInfo, AuthService};
use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use url::Url;
use crate::util::proof_key::{Challenge, Verifier};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub provider: OAuthProviderInfo,
    pub client: OAuthClientInfo,
}

impl OAuthConfig {
    pub fn api_endpoint(&self) -> url::Url {
        self.provider.api_endpoint.parse().unwrap()
    }
}

pub async fn handle_start_auth_request(
    redirect_uri: String,
    challenge: Challenge,
    auth_service: Arc<AuthService>,
    oauth_config: Arc<OAuthConfig>,
) -> Result<impl warp::Reply, anyhow::Error> {
    let url = Url::parse(&*redirect_uri)?;
    if url.scheme() != "http" {
        bail!("Redirect URI must have 'http' scheme.")
    }

    if url.host_str() != Some("127.0.0.1") && url.host_str() != Some("[::1]") {
        bail!("Host must be 127.0.0.1 or [::1].")
    }

    let auth_request = AuthRequestInfo {
        local_redirect: redirect_uri,
        challenge,
    };

    let token = auth_service.to_token(auth_request).await?;

    let redirect_uri = create_oauth_code_request_url(
        &*oauth_config,
        &["openid", "viewing_activity_read"],
        &token,
    )?.parse::<warp::http::Uri>()?;

    Ok(warp::redirect::temporary(redirect_uri))
}

pub async fn handle_oauth_callback(
    code: String,
    state: String,
    auth_service: Arc<AuthService>,
    auth_confirm_service: Arc<AuthConfirmService>,
) -> Result<impl warp::Reply, anyhow::Error> {
    let auth_req = auth_service.from_token(&state).await?;

    let confirm_info = AuthConfirmInfo {
        code,
        challenge: auth_req.challenge.clone(),
    };

    let token = auth_confirm_service.to_token(confirm_info).await?;

    let mut local_redirect_url = Url::parse(&auth_req.local_redirect)?;
    local_redirect_url
        .query_pairs_mut()
        .clear()
        .append_pair("token", &token);

    Ok(warp::redirect::temporary(warp::http::Uri::from(
        local_redirect_url.into_string().parse()?,
    )))
}

#[derive(Deserialize)]
pub struct TokenResponse {
    access_token: String,
    refresh_token: String,
    id_token: Option<String>,
    expires_in: u64,
    scope: Vec<String>,
    token_type: String,
}

pub async fn handle_confirm(
    token: String,
    verifier: Verifier,
    auth_confirm_service: Arc<AuthConfirmService>,
    oauth_config: Arc<OAuthConfig>,
) -> Result<TokenResponse, anyhow::Error> {
    let auth_complete_info = auth_confirm_service.from_token(&token).await?;
    crate::util::proof_key::verify_challenge(&auth_complete_info.challenge, &verifier)?;
    // Now that we're all verified, finish the key exchange

    let client = reqwest::Client::new();
    let response_text = client
        .post(&oauth_config.provider.token_endpoint)
        .query(&[
            ("client_id", &*oauth_config.client.client_id),
            ("client_secret", &*oauth_config.client.client_secret),
            ("code", &*auth_complete_info.code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", &*oauth_config.client.redirect_url),
        ])
        .send()
        .await?
        .text()
        .await?;

    Ok(serde_json::from_str::<TokenResponse>(&response_text)?)
}

fn create_oauth_code_request_url(
    config: &OAuthConfig,
    scopes: impl IntoIterator<Item = impl AsRef<str>>,
    state: &str,
) -> Result<String, anyhow::Error> {
    let mut authz_url = Url::parse(&config.provider.authz_endpoint)?;

    let v = scopes
        .into_iter()
        .map(|x| x.as_ref().to_string())
        .collect::<Vec<_>>();

    let scopes = v.join(" ");

    authz_url
        .query_pairs_mut()
        .clear()
        .append_pair("client_id", &config.client.client_id)
        .append_pair("redirect_uri", &config.client.redirect_url)
        .append_pair("scopes", &scopes)
        .append_pair("response_type", "code")
        .append_pair("state", state);

    Ok(authz_url.to_string())
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub scope: Vec<String>,
}

pub async fn refresh_oauth_token(
    refresh_token: &str,
    client: &reqwest::Client,
    oauth_config: &OAuthConfig,
) -> Result<RefreshResponse, anyhow::Error> {
    let resp_text = client
        .post(&oauth_config.provider.token_endpoint)
        .query(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &*oauth_config.client.client_id),
            ("client_secret", &*oauth_config.client.client_secret),
        ])
        .send()
        .await?
        .text()
        .await?;

    Ok(serde_json::from_str::<RefreshResponse>(&resp_text)?)
}
