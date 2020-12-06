#![allow(dead_code)]

mod channels;
mod config;
mod http_server;
mod net;
mod services;
mod util;

use config::oauth;
use serde::Deserialize;
use services::{fake::token_store, live::twitch_token};

use futures::prelude::*;

#[derive(Deserialize, Debug)]
struct EnvParams {
    server_addr: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let env_params = envy::prefixed("MINIBOT_").from_env::<EnvParams>()?;

    let twitch_client = envy::prefixed("MINIBOT_").from_env::<oauth::ClientInfo>()?;

    let twitch_config = oauth::Config::new(config::TWITCH_PROVIDER.clone(), twitch_client);

    let twitch_token_service = twitch_token::TwitchTokenHandle::new(twitch_config.clone());

    let (send, mut recv) = futures::channel::mpsc::channel(0);

    let router = http_server::authn::router(
        twitch_config.clone(),
        twitch_token_service,
        token_store::create(),
        Box::new(send),
    );

    tokio::spawn(async move { while let Some(_) = recv.next().await {} });

    let server = gotham::plain::init_server(env_params.server_addr.clone(), router);
    tokio::select! {
        _ = server => (),
        _ = tokio::signal::ctrl_c() => (),
    };

    Ok(())
}
