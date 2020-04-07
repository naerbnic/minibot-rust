#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    let (url, future) = minibot_client::run_client(&client, std::time::Instant::now() + std::time::Duration::from_secs(300), "", "");

    println!("Output: {:?}", webbrowser::open(&url.to_string())?);

    let access_token = future.await?;

    println!("Access token: {}", access_token);

    Ok(())
}
