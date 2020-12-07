pub mod fmt;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

macro_rules! getter {
    ($field:ident, $type:ty) => {
        pub fn $field(&self) -> &$type { &self.$field }
    };
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PostgresUser {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Postgres {
    pub hostname: String,
    pub port: u16,
    pub admin_user: PostgresUser,
    pub client_user: PostgresUser,
    pub db_name: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RabbitMq {
    pub address: String,
    pub port: u16,
    pub username: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OAuthClient {
    client_id: String,
    client_secret: String,
    redirect_url: String,
}

impl OAuthClient {
    getter!(client_id, str);
    getter!(client_secret, str);
    getter!(redirect_url, str);
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OAuthProvider {
    pub token_endpoint: String,
    pub authz_endpoint: String,
    pub jwks_keys_url: String,
}

impl OAuthProvider {
    getter!(token_endpoint, str);
    getter!(authz_endpoint, str);
    getter!(jwks_keys_url, str);
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OAuth {
    pub provider: OAuthProvider,
    pub client: OAuthClient,
}

impl OAuth {
    getter!(provider, OAuthProvider);
    getter!(client, OAuthClient);
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Minibot {
    pub address: String,
}

#[derive(Serialize, Deserialize)]
pub struct ConfigFile {
    pub postgres: Postgres,
    pub rabbitmq: RabbitMq,
    pub oauth_configs: BTreeMap<String, OAuth>,
    pub minibot: Minibot,
}
