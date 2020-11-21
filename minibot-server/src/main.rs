#![allow(dead_code)]

mod channels;
mod config;
mod http_server;
mod net;
mod services;
mod util;

use config::oauth;
use services::{fake::token_store, live::twitch_token};

use futures::prelude::*;

fn args() -> clap::App<'static, 'static> {
    use clap::{App, Arg};
    App::new("minibot-server").arg(
        Arg::with_name("dotenv")
            .long("dotenv")
            .value_name("FILE")
            .help("A .env file which environment variables will be drawn from.")
            .takes_value(true),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let matches = args().get_matches();

    if let Some(dotenv_path) = matches.value_of_os("dotenv") {
        dotenv::from_path(dotenv_path)?;
    }

    dotenv::from_path(dirs::home_dir().unwrap().join(".config/minibot-server/config.env"))?;

    let twitch_client = envy::prefixed("MINIBOT_").from_env::<oauth::ClientInfo>()?;

    let twitch_config = oauth::Config::new(config::TWITCH_PROVIDER.clone(), twitch_client);

    let twitch_token_service = twitch_token::TwitchTokenHandle::new(twitch_config.clone());

    let (send, mut recv) = futures::channel::mpsc::channel(10);

    let router = http_server::authn::router(
        twitch_config.clone(),
        twitch_token_service,
        token_store::create(),
        Box::new(send),
    );

    println!("Twitch config: {:#?}", twitch_config);

    tokio::spawn(async move { while let Some(_) = recv.next().await {} });

    let server = gotham::plain::init_server(("127.0.0.1", 5001), router);
    tokio::select! {
        _ = server => (),
        _ = tokio::signal::ctrl_c() => (),
    };

    Ok(())
}
