use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Id;
#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct CommandMessage {
    pub id: Id,
    pub method: String,
    pub payload: Value,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct CancelMessage {
    pub id: Id,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct ResponseMessage {
    pub id: Id,
    pub payload: Value,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct EndMessage {
    pub id: Id,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct ErrorMessage {
    pub error: String,
    pub id: Option<Id>,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "cmd")]
    Command(CommandMessage),
    #[serde(rename = "cancel")]
    Cancel(CancelMessage),
    #[serde(rename = "resp")]
    Response(ResponseMessage),
    #[serde(rename = "end")]
    End(EndMessage),
    #[serde(rename = "error")]
    Error(ErrorMessage),
}

impl Message {
    pub fn new_error_with_id<'a>(id: Id, msg: impl Into<Cow<'a, str>>) -> Self {
        Message::Error(ErrorMessage {
            id: Some(id),
            error: msg.into().into_owned(),
        })
    }
}
