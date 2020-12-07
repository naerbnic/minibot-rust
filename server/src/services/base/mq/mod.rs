use futures::channel::{mpsc::SendError, oneshot};
use futures::prelude::*;

use crate::util::id::Id;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Sender(#[from] SendError),

    #[error(transparent)]
    OneShot(#[from] oneshot::Canceled),
}

type MessageStream = Box<dyn Stream<Item = bytes::Bytes> + Send + 'static>;

#[non_exhaustive]
pub struct Subscription {
    pub sub_id: Id,
    pub stream: MessageStream,
}

#[derive(thiserror::Error, Debug)]
#[error("The message was not able to be sent.")]
pub struct PublishError;

#[async_trait::async_trait]
pub trait MessageBroker: Send {
    async fn subscribe(&mut self, channel_id: &str) -> Result<Subscription, Error>;
    async fn resume(&mut self, sub_id: Id) -> Result<Subscription, Error>;
    async fn publish(&mut self, channel_id: &str, body: bytes::Bytes) -> Result<(), PublishError>;
}
