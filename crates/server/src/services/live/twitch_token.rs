use crate::config::oauth;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug)]
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
    async fn exchange_code(&self, client: &Client, code: &str) -> anyhow::Result<TokenResponse>;
}

pub struct TwitchTokenImpl {
    config: oauth::Config,
}

impl TwitchTokenImpl {
    pub fn new(config: oauth::Config) -> Self {
        TwitchTokenImpl { config }
    }
}

#[async_trait::async_trait]
impl TwitchToken for TwitchTokenImpl {
    async fn exchange_code(&self, client: &Client, code: &str) -> anyhow::Result<TokenResponse> {
        #[derive(Serialize)]
        struct TokenQuery<'a> {
            client_id: &'a str,
            client_secret: &'a str,
            code: &'a str,
            grant_type: &'a str,
            redirect_uri: &'a str,
        }

        let response = client
            .post(self.config.provider().token_endpoint())
            .query(&TokenQuery {
                client_id: self.config.client().client_id(),
                client_secret: self.config.client().client_secret(),
                code,
                grant_type: "authorization_code",
                redirect_uri: self.config.client().redirect_url(),
            })
            .send()
            .await?;

        Ok(response.json().await?)
    }
}

pub type TwitchTokenService = dyn TwitchToken + Send + Sync + std::panic::RefUnwindSafe + 'static;

#[derive(Clone, gotham_derive::StateData)]
pub struct TwitchTokenHandle(Arc<TwitchTokenService>);

impl TwitchTokenHandle {
    pub fn new(config: oauth::Config) -> Self {
        TwitchTokenHandle(Arc::new(TwitchTokenImpl::new(config)))
    }
}

impl std::ops::Deref for TwitchTokenHandle {
    type Target = TwitchTokenService;
    fn deref(&self) -> &TwitchTokenService {
        &*self.0
    }
}
