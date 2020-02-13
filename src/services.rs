use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthRequestInfo {
    pub local_redirect: String,
    pub challenge: String,
}

#[async_trait]
pub trait AuthService: Sync {
    async fn request_to_token(&self, req: AuthRequestInfo) -> Result<String, anyhow::Error>;
    async fn token_to_request(&self, token: &str) -> Result<AuthRequestInfo, anyhow::Error>;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthConfirmInfo {
    pub code: String,
    pub challenge: String,
}

#[async_trait]
pub trait AuthConfirmService: Sync {
    async fn confirm_to_token(&self, req: AuthConfirmInfo) -> Result<String, anyhow::Error>;
    async fn token_to_confirm(&self, token: &str) -> Result<AuthConfirmInfo, anyhow::Error>;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IdentityInfo {
    twitch_id: String,
    twitch_auth_token: String
}
