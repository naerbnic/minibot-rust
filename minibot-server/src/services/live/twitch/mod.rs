pub mod throttled_token_source;
pub mod token_source;

use crate::config::oauth;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::Arc;

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum BroadcasterType {
    Normal,
    Partner,
    Affiliate,
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum UserType {
    Normal,
    Staff,
    Admin,
    GlobalMod,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TwitchUser {
    broadcaster_type: BroadcasterType,
    description: String,
    display_name: String,
    email: Option<String>,
    id: String,
    name: String,
    offline_image_url: String,
    profile_image_url: String,
    user_type: UserType,
    view_count: u64,
}

/// Many responses from twitch are wrapped in an object with a single "data" array field. This acts
/// as a wrapper for that.
#[derive(Clone, Serialize, Deserialize, Debug)]
struct DataWrapper<T> {
    data: Vec<T>,
}

impl<T> DataWrapper<T> {
    pub fn into_vec(self) -> Vec<T> {
        let DataWrapper { data } = self;
        data
    }
}

pub struct AuthToken {
    api_token: String,
}

#[async_trait::async_trait]
pub trait TwitchClient {
    async fn get_user_info(
        &self,
        auth_token: &AuthToken,
        id: &str,
    ) -> Result<TwitchUser, anyhow::Error>;
}

pub struct HttpTwitchClient<T> {
    client: T,
    config: Arc<oauth::Config>,
}

impl<T: AsRef<reqwest::Client> + Sync> HttpTwitchClient<T> {
    pub async fn call_api<Out: DeserializeOwned, Q: Serialize + ?Sized>(
        &self,
        auth_token: &AuthToken,
        method: reqwest::Method,
        path: &str,
        query_args: &Q,
    ) -> anyhow::Result<Out> {
        let client = self.client.as_ref();
        let endpoint = self.config.api_endpoint();
        Ok(client
            .request(method, &endpoint.join(path).unwrap().to_string())
            .header("Authorization", format!("Bearer {}", auth_token.api_token))
            .query(query_args)
            .send()
            .await?
            .json::<Out>()
            .await?)
    }
}

#[async_trait::async_trait]
impl<T: AsRef<reqwest::Client> + Sync> TwitchClient for HttpTwitchClient<T> {
    async fn get_user_info(&self, auth_token: &AuthToken, id: &str) -> anyhow::Result<TwitchUser> {
        let mut users = self
            .call_api::<DataWrapper<TwitchUser>, _>(
                auth_token,
                reqwest::Method::GET,
                "helix/users",
                &[("id", id)],
            )
            .await?
            .into_vec();

        anyhow::ensure!(users.len() == 1, "Expected a single user to be returned");

        Ok(users.pop().unwrap())
    }
}
