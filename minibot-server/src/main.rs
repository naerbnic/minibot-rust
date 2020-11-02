#![allow(dead_code)]

mod config;
mod http_server;
mod net;
mod services;
mod util;

use config::oauth;
use services::{
    base::{token_service::TokenServiceHandle, twitch_token},
    fake::token_service::create_serde,
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

    let twitch_client = ds
        .read_from("twitch-client.json")
        .with_format(devsecrets::JsonFormat)
        .into_value::<oauth::ClientInfo>()
        .expect("Secret is readable");

    let twitch_config = oauth::Config::new(config::TWITCH_PROVIDER.clone(), twitch_client);

    let auth_service: TokenServiceHandle<AuthRequestInfo> = create_serde();
    let auth_confirm_service: TokenServiceHandle<AuthConfirmInfo> = create_serde();
    let twitch_token_service = twitch_token::TwitchTokenHandle::new(twitch_config.clone());

    let (send, mut recv) = futures::channel::mpsc::channel(10);

    let router = http_server::endpoints::router(
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
