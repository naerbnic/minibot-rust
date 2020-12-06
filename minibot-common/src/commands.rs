use serde::{Serialize, Deserialize};

use crate::net::rpc::Command;

#[derive(Serialize)]
pub struct GetUserId;

#[derive(Deserialize)]
pub struct GetUserIdResponse {
    user_id: u64,
}

impl GetUserIdResponse {
    pub fn user_id(&self) -> u64 {
        self.user_id
    }
}

impl Command for GetUserId {
    type Response = GetUserIdResponse;

    fn method() -> &'static str {
        "user_id"
    }
}