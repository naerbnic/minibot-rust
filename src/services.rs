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
