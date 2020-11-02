use crate::util::id::{Id, IdGen};
use futures::lock::Mutex;
use std::collections::{BTreeMap, BTreeSet};

use futures::channel::{
    mpsc::{channel, Receiver, SendError, Sender},
    oneshot,
};
use futures::prelude::*;

use crate::util::future::opt_cell::{opt_cell, OptCellReplacer};

use crate::services::mq::{Error, MessageBroker, PublishError, Subscription};

pub struct Message {
    base: MessageBase,
}

impl std::ops::Deref for Message {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &*self.base.body
    }
}

#[derive(Clone)]
pub struct MessageBase {
    body: bytes::Bytes,
}

enum Event {
    PublishMessage {
        channel: String,
        body: bytes::Bytes,
    },

    Subscribe {
        channel: String,
        id_send: oneshot::Sender<Id>,
        output: Sender<MessageBase>,
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
    async fn subscribe(&mut self, channel_id: &str) -> Result<Subscription, Error> {
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

        let stream = msg_recv.map(|base| base.body);

        Ok(Subscription {
            sub_id,
            stream: Box::new(stream),
        })
    }

    async fn resume(&mut self, _sub_id: Id) -> Result<Subscription, Error> {
        todo!()
    }

    async fn publish(&mut self, channel_id: &str, body: bytes::Bytes) -> Result<(), PublishError> {
        self.event_channel
            .send(Event::PublishMessage {
                channel: channel_id.to_string(),
                body,
            })
            .await
            .map_err(|_| PublishError)?;

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
                let sub_id = state.add_subscriber(&channel, output).await;
                let _ = id_send.send(sub_id);
            }
        }
    }
}

struct BrokerQueue {
    subscribers: BTreeSet<Id>,
}

impl BrokerQueue {
    pub fn new() -> Self {
        BrokerQueue {
            subscribers: BTreeSet::new(),
        }
    }
}

struct SubscriptionState {
    topic: String,
    message_sink: Mutex<Sender<MessageBase>>,
    replacer: Mutex<OptCellReplacer<Sender<MessageBase>>>,
}

impl SubscriptionState {
    pub fn new(topic: String) -> Self {
        let (send, mut recv) = channel::<MessageBase>(10);
        let (mut cell, replacer) = opt_cell::<Sender<MessageBase>>();

        tokio::spawn({
            async move {
                while let Some(msg) = recv.next().await {
                    loop {
                        let borrow_or_timeout = tokio::time::timeout(
                            std::time::Duration::from_secs(5 * 60),
                            cell.borrow(),
                        )
                        .await;

                        let borrow_result = match borrow_or_timeout {
                            Err(_) => {
                                // We timed out waiting for the borrow.
                                return;
                            }
                            Ok(r) => r,
                        };

                        if let Ok(output) = borrow_result {
                            if let Err(_) = output.send(msg.clone()).await {
                                // The socket got closed on us. We still have a message, so drop
                                // the sender and wait for another one.
                                cell.drop_value();
                            } else {
                                break;
                            }
                        } else {
                            // The subscription state itself was dropped. Break out of the loop
                            // entirely. This will drop recv, and propagate the change up.
                            return;
                        }
                    }
                }
            }
        });

        SubscriptionState {
            topic,
            message_sink: Mutex::new(send),
            replacer: Mutex::new(replacer),
        }
    }

    pub async fn publish(&self, body: MessageBase) -> Result<(), SendError> {
        let mut guard = self.message_sink.lock().await;
        guard.send(body).await
    }

    pub async fn replace(&self, sender: Sender<MessageBase>) {
        let mut guard = self.replacer.lock().await;
        guard.replace(sender).await.unwrap();
    }
}

struct BrokerState {
    topics: BTreeMap<String, BrokerQueue>,
    subscriptions: BTreeMap<Id, SubscriptionState>,
    sub_id_gen: IdGen,
}

impl BrokerState {
    pub fn new() -> Self {
        BrokerState {
            topics: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            sub_id_gen: IdGen::new(),
        }
    }
    pub async fn add_subscriber(&mut self, channel: &str, listener: Sender<MessageBase>) -> Id {
        let new_id = self.sub_id_gen.gen_id();
        let sub_state = SubscriptionState::new(channel.to_string());
        sub_state.replace(listener).await;
        self.subscriptions.insert(new_id.clone(), sub_state);

        self.topics
            .entry(channel.to_string())
            .or_insert_with(BrokerQueue::new)
            .subscribers
            .insert(new_id.clone());

        new_id
    }

    pub async fn publish_message(&mut self, channel: &str, body: bytes::Bytes) {
        if let Some(queue) = self.topics.get_mut(channel) {
            for sub_id in &queue.subscribers {
                self.subscriptions
                    .get_mut(sub_id)
                    .unwrap()
                    .publish(MessageBase { body: body.clone() })
                    .await
                    .unwrap();
            }
        }
    }
}
