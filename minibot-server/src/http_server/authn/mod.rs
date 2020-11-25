use serde::{Deserialize, Serialize};

mod endpoints;
mod handlers;

pub use endpoints::router;

#[derive(Clone, Serialize, Deserialize)]
struct IdentityInfo {
    twitch_id: String,
    twitch_auth_token: String,
}