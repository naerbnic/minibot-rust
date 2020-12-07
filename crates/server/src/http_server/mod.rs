pub mod middleware;
pub mod authn;
pub mod channel;

use serde::{Serialize, Deserialize};

/// A common token type that represents a valid ID
#[derive(Clone, Serialize, Deserialize)]
pub struct IdToken {
    id: u64,
}

impl IdToken {
    pub fn id(&self) -> u64 { self.id }
}

impl crate::services::base::token_store::TokenData for IdToken {}
