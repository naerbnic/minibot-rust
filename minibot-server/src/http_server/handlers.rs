use crate::config::oauth;
use crate::services::{token_service::TokenService, AuthConfirmInfo, AuthRequestInfo};
use anyhow::bail;
use minibot_common::proof_key;
use serde::{Deserialize, Serialize};
use url::Url;

pub async fn handle_start_auth_request(
    redirect_uri: String,
    challenge: proof_key::Challenge,
    auth_service: &dyn TokenService<AuthRequestInfo>,
    oauth_config: &oauth::Config,
) -> Result<String, anyhow::Error> {
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
    )?;

    Ok(redirect_uri)
}

pub async fn handle_oauth_callback(
    code: String,
    state: String,
    auth_service: &dyn TokenService<AuthRequestInfo>,
    auth_confirm_service: &dyn TokenService<AuthConfirmInfo>,
) -> Result<String, anyhow::Error> {
    let auth_req = match auth_service.from_token(&state).await? {
        Some(auth_req) => auth_req,
        None => anyhow::bail!("Could not retrieve token."),
    };

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

    Ok(local_redirect_url.into_string())
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
    client: &reqwest::Client,
    token: String,
    verifier: proof_key::Verifier,
    auth_confirm_service: &dyn TokenService<AuthConfirmInfo>,
    oauth_config: &oauth::Config,
) -> Result<TokenResponse, anyhow::Error> {
    let auth_complete_info = match auth_confirm_service.from_token(&token).await? {
        Some(info) => info,
        None => anyhow::bail!("Could not retrieve token."),
    };
    verifier.verify(&auth_complete_info.challenge)?;
    // Now that we're all verified, finish the key exchange

    let response_text = client
        .post(oauth_config.provider().token_endpoint())
        .query(&[
            ("client_id", &*oauth_config.client().client_id()),
            ("client_secret", &*oauth_config.client().client_secret()),
            ("code", &*auth_complete_info.code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", &*oauth_config.client().redirect_url()),
        ])
        .send()
        .await?
        .text()
        .await?;

    Ok(serde_json::from_str::<TokenResponse>(&response_text)?)
}

fn create_oauth_code_request_url(
    config: &oauth::Config,
    scopes: impl IntoIterator<Item = impl AsRef<str>>,
    state: &str,
) -> Result<String, anyhow::Error> {
    let mut authz_url = Url::parse(&config.provider().authz_endpoint())?;

    let v = scopes
        .into_iter()
        .map(|x| x.as_ref().to_string())
        .collect::<Vec<_>>();

    let scopes = v.join(" ");

    authz_url
        .query_pairs_mut()
        .clear()
        .append_pair("client_id", &config.client().client_id())
        .append_pair("redirect_uri", &config.client().redirect_url())
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
    oauth_config: &oauth::Config,
) -> Result<RefreshResponse, anyhow::Error> {
    let resp_text = client
        .post(oauth_config.provider().token_endpoint())
        .query(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &*oauth_config.client().client_id()),
            ("client_secret", &*oauth_config.client().client_secret()),
        ])
        .send()
        .await?
        .text()
        .await?;

    Ok(serde_json::from_str::<RefreshResponse>(&resp_text)?)
}
