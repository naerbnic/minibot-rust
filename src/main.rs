#![allow(dead_code)]

mod config;
mod filters;
mod handlers;
mod services;
mod util;

use handlers::{OAuthClientInfo, OAuthConfig};
use services::{AuthConfirmService, AuthService, SerdeTokenService};
use std::sync::Arc;

devsecrets::import_id!(DEVSECRETS_ID);

#[tokio::main]
async fn main() {
    env_logger::init();
    // Match any request and return hello world!
    // let routes = warp::any().map(|| "Hello, World!");

    // warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    let ds = devsecrets::DevSecrets::from_id(&DEVSECRETS_ID)
        .unwrap()
        .unwrap();

    let twitch_client: OAuthClientInfo = ds
        .read_from("twitch-client.json")
        .with_format(devsecrets::JsonFormat)
        .into_value()
        .expect("Secret is readable");

    let twitch_config = OAuthConfig {
        client: twitch_client,
        provider: config::TWITCH_PROVIDER.clone(),
    };

    let auth_service: Arc<AuthService> = SerdeTokenService::new();
    let auth_confirm_service: Arc<AuthConfirmService> = SerdeTokenService::new();

    println!("Twitch config: {:#?}", twitch_config);
}
