use clap::{App, Arg};
use minibot_client::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = App::new("minibot-client-cli")
        .arg(Arg::with_name("SERVER").required(true))
        .get_matches();

    let server = Server::new(matches.value_of("SERVER").unwrap());

    eprintln!("Trying to start server...");

    let client_authn = server
        .authenticate(
            std::time::Instant::now() + std::time::Duration::from_secs(300),
            |url| {
                eprintln!("Go to URL {}", url);
                Ok::<_, std::convert::Infallible>(())
            },
        )
        .await?;

    eprintln!("Client Authentication: {:?}", client_authn);

    let _connect = server.connect(&client_authn).await?;

    eprintln!("Connected!");

    Ok(())
}
