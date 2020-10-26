use std::collections::{BTreeMap, VecDeque};

use futures::prelude::*;
use futures::{
    channel::{
        mpsc::{channel, Receiver, SendError, Sender},
        oneshot,
    },
    future::join_all,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Sender(#[from] SendError),

    #[error(transparent)]
    OneShot(#[from] oneshot::Canceled),
}
pub struct Message {
    channel: String,
    base: MessageBase,
    sub_id: u32,
    event_sink: Option<Sender<Event>>,
}

impl Message {
    pub async fn ack(mut self) -> Result<(), Error> {
        let mut sender = self.event_sink.take().unwrap();

        sender.send(self.create_ack_event()).await?;

        Ok(())
    }

    fn create_ack_event(&self) -> Event {
        Event::Ack {
            channel: self.channel.clone(),
            msg_id: self.base.msg_id,
            sub_id: self.sub_id,
        }
    }
}

impl std::ops::Deref for Message {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &*self.base.body
    }
}

impl Drop for Message {
    fn drop(&mut self) {
        if let Some(mut sender) = self.event_sink.take() {
            let ack_event = self.create_ack_event();
            tokio::spawn(async move { sender.send(ack_event).await });
        }
    }
}

#[derive(Clone)]
pub struct MessageBase {
    msg_id: u32,
    body: bytes::Bytes,
}

type MessageStream = Box<dyn Stream<Item = Message> + Send + 'static>;

#[async_trait::async_trait]
pub trait MessageBroker: Send {
    async fn subscribe(&mut self, channel_id: &str) -> Result<MessageStream, Error>;
    async fn publish(&mut self, channel_id: &str, body: &[u8]) -> Result<(), Error>;
}

// In-memory fake implementation.

enum Event {
    PublishMessage {
        channel: String,
        body: bytes::Bytes,
    },

    Subscribe {
        channel: String,
        id_send: oneshot::Sender<u32>,
        output: Sender<MessageBase>,
    },

    Ack {
        channel: String,
        sub_id: u32,
        msg_id: u32,
    },
}

pub struct InMemoryMessageBroker {
    event_channel: Sender<Event>,
}

impl InMemoryMessageBroker {
    pub fn new() -> Self {
        let (event_send, event_recv) = channel(10);

        tokio::spawn(run_message_broker_event_loop(event_recv));

        InMemoryMessageBroker {
            event_channel: event_send,
        }
    }
}

#[async_trait::async_trait]
impl MessageBroker for InMemoryMessageBroker {
    async fn subscribe(&mut self, channel_id: &str) -> Result<MessageStream, Error> {
        let (id_send, id_recv) = oneshot::channel();
        let (msg_send, msg_recv) = channel(10);
        self.event_channel
            .send(Event::Subscribe {
                channel: channel_id.to_string(),
                id_send,
                output: msg_send,
            })
            .await?;

        let sub_id = id_recv.await?;

        let stream = msg_recv.map({
            let channel = channel_id.to_string();
            let event_channel = self.event_channel.clone();
            move |base| Message {
                base,
                channel: channel.clone(),
                sub_id,
                event_sink: Some(event_channel.clone()),
            }
        });

        Ok(Box::new(stream))
    }

    async fn publish(&mut self, channel_id: &str, body: &[u8]) -> Result<(), Error> {
        self.event_channel
            .send(Event::PublishMessage {
                channel: channel_id.to_string(),
                body: bytes::Bytes::copy_from_slice(body),
            })
            .await?;

        Ok(())
    }
}

async fn run_message_broker_event_loop(mut event_stream: Receiver<Event>) {
    let mut state = BrokerState::new();
    while let Some(event) = event_stream.next().await {
        match event {
            Event::PublishMessage { channel, body } => state.publish_message(&channel, body).await,
            Event::Subscribe {
                channel,
                id_send,
                output,
            } => {
                let sub_id = state.add_listener(&channel, output).await;
                let _ = id_send.send(sub_id);
            }
            Event::Ack {
                channel,
                sub_id,
                msg_id,
            } => state.ack_message(&channel, sub_id, msg_id).await,
        }
    }
}

struct BrokerQueue {
    backlog: VecDeque<bytes::Bytes>,
    listeners: BTreeMap<u32, Sender<MessageBase>>,
    next_sub: u32,
    next_msg: u32,
}

impl BrokerQueue {
    pub fn new() -> Self {
        BrokerQueue {
            backlog: VecDeque::new(),
            listeners: BTreeMap::new(),
            next_sub: 0,
            next_msg: 0,
        }
    }

    pub fn add_listener(&mut self, listener: Sender<MessageBase>) -> u32 {
        let sub_id = self.next_sub;
        self.next_sub += 1;

        assert!(self.listeners.insert(sub_id, listener).is_none());
        sub_id
    }

    pub async fn publish_message(&mut self, body: bytes::Bytes) {
        let msg_id = self.next_msg;
        self.next_msg += 1;

        let message_base = MessageBase { msg_id, body };

        let results = join_all(
            self.listeners
                .iter_mut()
                .map(move |(&id, sender)| sender.send(message_base.clone()).map(move |r| (id, r))),
        )
        .await;

        for (id, r) in results {
            if let Err(_) = r {
                self.listeners.remove(&id);
            }
        }
    }

    pub async fn ack_message(&mut self, _sub_id: u32, _msg_id: u32) {
        todo!()
    }
}

struct BrokerState {
    queues: BTreeMap<String, BrokerQueue>,
}

impl BrokerState {
    pub fn new() -> Self {
        BrokerState {
            queues: BTreeMap::new(),
        }
    }
    pub async fn add_listener(&mut self, channel: &str, listener: Sender<MessageBase>) -> u32 {
        self.queues
            .entry(channel.to_string())
            .or_insert_with(BrokerQueue::new)
            .add_listener(listener)
    }

    pub async fn publish_message(&mut self, channel: &str, body: bytes::Bytes) {
        if let Some(queue) = self.queues.get_mut(channel) {
            queue.publish_message(body).await
        }
    }

    pub async fn ack_message(&mut self, channel: &str, sub_id: u32, msg_id: u32) {
        if let Some(queue) = self.queues.get_mut(channel) {
            queue.ack_message(sub_id, msg_id).await
        }
    }
}
