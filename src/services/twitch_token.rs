use reqwest::Client;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::handlers::OAuthConfig;

#[derive(Deserialize, Debug)]
pub struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    scope: Option<Vec<String>>,
    id_token: Option<String>,
    token_type: String,
}

#[async_trait::async_trait]
pub trait TwitchToken {
    async fn exchange_code(&self, code: &str) -> anyhow::Result<TokenResponse>;
}

pub struct TwitchTokenImpl {
    client: Arc<Client>,
    config: Arc<OAuthConfig>
}

impl TwitchTokenImpl {
    pub fn new(client: Arc<Client>, config: Arc<OAuthConfig>) -> Self {
        TwitchTokenImpl {
            client, config
        }
    }
}

#[async_trait::async_trait]
impl TwitchToken for TwitchTokenImpl {
    async fn exchange_code(&self, code: &str) -> anyhow::Result<TokenResponse> {
        #[derive(Serialize)]
        struct TokenQuery<'a> {
            client_id: &'a str,
            client_secret: &'a str,
            code: &'a str,
            grant_type: &'a str,
            redirect_uri: &'a str,
        }

        let response = self.client.post(&self.config.provider.token_endpoint).query(&TokenQuery {
            client_id: &self.config.client.client_id,
            client_secret: &self.config.client.client_secret,
            code,
            grant_type: "authorization_code",
            redirect_uri: &self.config.client.redirect_url,
        }).send().await?;

        Ok(response.json().await?)
    }
}

pub type TwitchTokenService = dyn TwitchToken + Send + Sync + 'static;

pub fn create_service(client: Arc<Client>, config: Arc<OAuthConfig>) -> Arc<TwitchTokenService> {
    Arc::new(TwitchTokenImpl::new(client, config))
}
