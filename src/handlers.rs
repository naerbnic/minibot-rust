use crate::services::{AuthConfirmInfo, AuthConfirmService, AuthRequestInfo, AuthService};
use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderInfo {
    pub token_endpoint: String,
    pub authz_endpoint: String,
    pub jwks_keys_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClientInfo {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_utl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub provider: OAuthProviderInfo,
    pub client: OAuthClientInfo,
}

pub async fn handle_start_auth_request(
    redirect_uri: String,
    challenge: String,
    auth_service: Arc<dyn AuthService>,
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

    let token = auth_service.request_to_token(auth_request).await?;

    Ok(create_oauth_code_request_url(
        &*oauth_config,
        &["openid"],
        &token,
    )?)
}

pub async fn handle_oauth_callback(
    code: String,
    state: String,
    auth_service: Arc<dyn AuthService>,
    auth_confirm_service: Arc<dyn AuthConfirmService>,
) -> Result<impl warp::Reply, anyhow::Error> {
    let auth_req = auth_service.token_to_request(&state).await?;

    let confirm_info = AuthConfirmInfo {
        code,
        challenge: auth_req.challenge.clone(),
    };

    let token = auth_confirm_service.confirm_to_token(confirm_info).await?;

    let mut local_redirect_url = Url::parse(&auth_req.local_redirect)?;
    local_redirect_url
        .query_pairs_mut()
        .clear()
        .append_pair("token", &token);

    Ok(warp::redirect::temporary(warp::http::Uri::from(
        local_redirect_url.into_string().parse()?,
    )))
}

pub async fn handle_confirm(
    token: String,
    verifier: String,
    auth_confirm_service: Arc<dyn AuthConfirmService>,
) -> Result<impl warp::Reply, anyhow::Error> {
    Ok("Hello, World!")
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
        .append_pair("redirect_uri", &config.client.redirect_utl)
        .append_pair("scopes", &scopes)
        .append_pair("response_type", "code")
        .append_pair("state", state);

    Ok(authz_url.to_string())
}
