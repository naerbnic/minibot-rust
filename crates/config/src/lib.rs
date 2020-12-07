pub mod fmt;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OAuthProvider {
    pub token_endpoint: String,
    pub authz_endpoint: String,
    pub jwks_keys_url: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OAuth {
    pub provider: OAuthProvider,
    pub client: OAuthClient,
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
