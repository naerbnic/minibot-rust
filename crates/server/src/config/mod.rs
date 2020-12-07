pub mod oauth;

lazy_static::lazy_static! {
    pub static ref TWITCH_PROVIDER: oauth::ProviderInfo =
        serde_json::from_str(std::include_str!("twitch-provider.json")).unwrap();
}
