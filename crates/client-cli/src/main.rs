use clap::{App, Arg};
use minibot_client::Server;
use std::io::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = App::new("minibot-client-cli")
        .arg(Arg::with_name("SERVER").required(true))
        .get_matches();

    let server = Server::new(matches.value_of("SERVER").unwrap());

    eprintln!("Trying to start server...");

    let (auth_url, exchanger) = server.authenticate_token().await;

    println!("Go to the following URL: {}", auth_url);

    print!("Paste the confirmation code you received at the end of the process here: ");
    std::io::stdout().flush()?;
    let mut code = String::new();
    std::io::stdin().read_line(&mut code)?;
    let code = code.strip_suffix("\n").unwrap();
    
    let client_authn = exchanger.exchange(code).await?;

    eprintln!("Client Authentication: {:?}", client_authn);

    let _connect = server.connect(&client_authn).await?;

    eprintln!("Connected!");

    Ok(())
}
