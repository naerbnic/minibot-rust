use clap::{App, Arg};
use url::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = App::new("minibot-client-cli")
        .arg(Arg::with_name("SERVER").required(true))
        .get_matches();

    println!("Matches: {:?}", matches);

    let server_url = Url::parse(matches.value_of("SERVER").unwrap())?;

    let mut auth_url = server_url.clone();
    auth_url.set_path("/login");
    let mut confirm_url = server_url.clone();
    confirm_url.set_path("/confirm");
    let confirm_url = confirm_url.to_string();

    let client = reqwest::Client::new();

    let (url, future) = minibot_client::run_client(
        &client,
        std::time::Instant::now() + std::time::Duration::from_secs(300),
        &auth_url.to_string(),
        &confirm_url,
    );

    println!("Output: {:?}", webbrowser::open(&url.to_string())?);

    let access_token = future.await?;

    println!("Access token: {}", access_token);

    Ok(())
}
