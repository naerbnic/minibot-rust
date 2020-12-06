use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct PostgresUser {
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct Postgres {
    address: String,
    admin_user: PostgresUser,
    client_user: PostgresUser,
    db_name: String,
}

#[derive(Serialize, Deserialize)]
struct RabbitMq {
    address: String,
    port: u16,
    username: String,
}

#[derive(Serialize, Deserialize)]
struct OAuthClient {
    client_id: String,
    client_secret: String,
    redirect_url: String,
}

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    postgres: Postgres,
    rabbitmq: RabbitMq,
    twitch_oauth_client: OAuthClient,
}
