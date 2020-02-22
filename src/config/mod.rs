use crate::handlers::OAuthProviderInfo;

lazy_static::lazy_static! {
    pub static ref TWITCH_PROVIDER: OAuthProviderInfo =
        serde_json::from_str(std::include_str!("twitch-provider.json")).unwrap();
}
