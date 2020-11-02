#![allow(dead_code)]

mod config;
mod endpoints;
mod filters;
mod handlers;
mod net;
mod reqwest_middleware;
mod services;
mod util;

use handlers::{OAuthClientInfo, OAuthConfig};
use services::{
    fake::token_service::create_serde, token_service::TokenServiceHandle, twitch_token,
    AuthConfirmInfo, AuthRequestInfo,
};

use futures::prelude::*;

#[tokio::main]
async fn main() {
    env_logger::init();
    // Match any request and return hello world!
    // let routes = warp::any().map(|| "Hello, World!");

    // warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    let ds = devsecrets::DevSecrets::from_id(&devsecrets::import_id!()).unwrap();

    let twitch_client: OAuthClientInfo = ds
        .read_from("twitch-client.json")
        .with_format(devsecrets::JsonFormat)
        .into_value()
        .expect("Secret is readable");

    let twitch_config = OAuthConfig {
        client: twitch_client,
        provider: config::TWITCH_PROVIDER.clone(),
    };

    let auth_service: TokenServiceHandle<AuthRequestInfo> = create_serde();
    let auth_confirm_service: TokenServiceHandle<AuthConfirmInfo> = create_serde();
    let twitch_token_service = twitch_token::TwitchTokenHandle::new(twitch_config.clone());

    let (send, mut recv) = futures::channel::mpsc::channel(10);

    let router = endpoints::router(
        twitch_config.clone(),
        twitch_token_service,
        auth_service,
        auth_confirm_service,
        Box::new(send),
    );

    println!("Twitch config: {:#?}", twitch_config);

    tokio::spawn(async move { while let Some(_) = recv.next().await {} });

    let server = gotham::plain::init_server(("127.0.0.1", 5001), router);
    tokio::select! {
        _ = server => (),
        _ = tokio::signal::ctrl_c() => (),
    };
}
