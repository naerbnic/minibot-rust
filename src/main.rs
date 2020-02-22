#![allow(dead_code)]

mod filters;
mod handlers;
mod services;
mod util;
mod config;

devsecrets::devsecrets_config! {
    static DEVSECRETS;
}

#[tokio::main]
async fn main() {
    env_logger::init();
    // Match any request and return hello world!
    // let routes = warp::any().map(|| "Hello, World!");

    // warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    let twitch_client: crate::handlers::OAuthClientInfo = DEVSECRETS
        .read_json_secret("twitch-client.json")
        .expect("Secret is readable");
    
    println!("Twitch client: {:#?}", twitch_client);
    println!("Twitch provider: {:#?}", &*config::TWITCH_PROVIDER);
}
