use anyhow::bail;
use minibot_common::proof_key;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{config::oauth, services::base::token_store::TokenStoreHandle};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "auth_method")]
pub enum AuthMethod {
    #[serde(rename = "local_http")]
    LocalHttp { redirect_uri: String },
    #[serde(rename = "token")]
    Token,
}

/// Info stored between the post to the minibot auth exchange start and the
/// OAuth2 redirect response.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AuthRequestInfo {
    /// The verification method desired by the client
    pub auth_method: AuthMethod,

    /// The challenge string provided by a user.
    pub challenge: proof_key::Challenge,
}

impl crate::services::base::token_store::TokenData for AuthRequestInfo {}

/// Info stored between returning the token via redirect to the user and the
/// user submitting the token to the account-create/bot-add endpoint with the
/// challenge verifier
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AuthConfirmInfo {
    /// The code returned by the OAuth2 provider that can be exchanged for a
    /// token.
    pub code: String,

    /// The challenge provided by the user. By providing a verifier, it
    /// ensures that the final use of the token on the endpoint is from the
    /// person who requested it.
    pub challenge: proof_key::Challenge,
}

impl crate::services::base::token_store::TokenData for AuthConfirmInfo {}

pub async fn handle_start_auth_request(
    auth_method: AuthMethod,
    challenge: proof_key::Challenge,
    token_store: &TokenStoreHandle,
    oauth_config: &oauth::Config,
) -> Result<String, anyhow::Error> {
    match &auth_method {
        AuthMethod::LocalHttp { redirect_uri } => {
            let url = Url::parse(&redirect_uri)?;
            if url.scheme() != "http" {
                bail!("Redirect URI must have 'http' scheme.")
            }

            if url.host_str() != Some("127.0.0.1") && url.host_str() != Some("[::1]") {
                bail!("Host must be 127.0.0.1 or [::1].")
            }
        }
        _ => {}
    }

    let auth_request = AuthRequestInfo {
        auth_method,
        challenge,
    };

    let token = token_store.to_token(&auth_request).await?;

    let redirect_uri = create_oauth_code_request_url(
        &*oauth_config,
        &["openid", "viewing_activity_read"],
        &token,
    )?;

    Ok(redirect_uri)
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
    token_store: &TokenStoreHandle,
    oauth_config: &oauth::Config,
) -> Result<TokenResponse, anyhow::Error> {
    let auth_complete_info: AuthConfirmInfo = token_store
        .from_token(&token)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Could not retrieve token."))?;
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
