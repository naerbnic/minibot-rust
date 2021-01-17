#[cfg(test)]
mod test;

mod pool;

use futures::stream::BoxStream;
use futures::stream::StreamExt;
use lapin::{
    options::{ExchangeDeclareOptions, QueueDeclareOptions},
    types::{AMQPValue, FieldTable, ShortString},
    Connection, ExchangeKind,
};
use std::convert::TryInto;
use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Other error: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl Error {
    pub fn new_other<E: std::error::Error + Send + Sync + 'static>(err: E) -> Self {
        Error::Other(Box::new(err))
    }
}

#[non_exhaustive]
pub enum MessageSource {
    User(String),
}

impl MessageSource {
    fn to_routing_key(&self) -> String {
        match self {
            MessageSource::User(username) => format!("irc:{}", username),
        }
    }
}

pub struct Message(Vec<u8>);

impl Message {
    pub fn new(data: &[u8]) -> Self {
        Message(data.to_vec())
    }
    pub fn data(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct QueueId(Uuid);

impl QueueId {
    fn new() -> Self {
        QueueId(Uuid::new_v4())
    }

    fn queue_name(&self) -> String {
        format!("recv_queue:{}", self.0.to_hyphenated().to_string())
    }
}

pub struct Queue {
    id: QueueId,
    consumer: lapin::Consumer,
}

impl Queue {
    pub fn id(&self) -> &QueueId {
        &self.id
    }

    pub fn into_stream(self) -> BoxStream<'static, Message> {
        self.consumer
            .filter_map(|value| async move {
                if let Ok((_, mut delivery)) = value {
                    let _ = delivery.acker.ack(Default::default()).await;
                    let data = std::mem::replace(&mut delivery.data, Vec::new());
                    Some(Message(data))
                } else {
                    None
                }
            })
            .boxed()
    }
}

const PRIMARY_EXCHANGE: &str = "primary_exchange";

pub struct Broker {
    conn: std::sync::Arc<Connection>,
}

impl Broker {
    // Create a new broker from a suitable AMQP URI. Host is expected to either be empty, or
    // have previously used to create a broker of this type.
    pub async fn new(uri: &str) -> Result<Self, Error> {
        let conn = Connection::connect(&uri, Default::default())
            .await
            .map_err(Error::new_other)?;

        // Ensure the primary exchange is available
        let broker = Broker {
            conn: std::sync::Arc::new(conn),
        };

        let channel = broker.create_channel().await?;
        channel
            .exchange_declare(
                PRIMARY_EXCHANGE,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    auto_delete: false,
                    durable: true,
                    ..Default::default()
                },
                Default::default(),
            )
            .await
            .map_err(Error::new_other)?;

        Ok(broker)
    }

    async fn create_channel(&self) -> Result<lapin::Channel, Error> {
        self.conn.create_channel().await.map_err(Error::new_other)
    }

    // Create a fresh queue getting messages from the given source.
    //
    // # Arguments
    //
    // * `source` - A [`MessageSource`] describing where the messages should come from.
    // * `expires` - The amount of time a queue will spend idle before deleting itself.
    //
    // # Returns
    //
    // Either a [`Queue`] object, which allows the messages to be read, and containing the ID, or
    // an [`Error`] that gives the reason the queue failed to be created.
    pub async fn create_queue(
        &self,
        source: &MessageSource,
        expires: std::time::Duration,
    ) -> Result<Queue, Error> {
        let channel = self.create_channel().await?;

        // Check that there is an existing fanout. These should be idempotent, so there should be
        // no issues with race conditions
        let fanout_name = format!("fanout_excg:{}", source.to_routing_key());
        channel
            .exchange_declare(
                &fanout_name,
                ExchangeKind::Fanout,
                ExchangeDeclareOptions {
                    auto_delete: true,
                    ..Default::default()
                },
                Default::default(),
            )
            .await
            .map_err(Error::new_other)?;

        channel
            .exchange_bind(
                &fanout_name,
                PRIMARY_EXCHANGE,
                &source.to_routing_key(),
                Default::default(),
                Default::default(),
            )
            .await
            .map_err(Error::new_other)?;

        // Create new queue. Uuid guarantees this is unique.
        let new_id = QueueId::new();
        let queue_name = new_id.queue_name();
        let opts = QueueDeclareOptions {
            ..Default::default()
        };
        let mut fields = FieldTable::default();
        fields.insert(
            ShortString::from("x-expires"),
            AMQPValue::LongLongInt(expires.as_millis().try_into().unwrap()),
        );
        channel
            .queue_declare(&queue_name, opts, fields)
            .await
            .map_err(Error::new_other)?;

        // Bind to fanout above.
        channel
            .queue_bind(
                &queue_name,
                &fanout_name,
                "", // Fanouts have no routing key
                Default::default(),
                Default::default(),
            )
            .await
            .map_err(Error::new_other)?;

        // Open a stream from the queue to read from.

        let consumer = channel
            .basic_consume(&queue_name, "", Default::default(), Default::default())
            .await
            .map_err(Error::new_other)?;
        Ok(Queue {
            id: new_id.clone(),
            consumer,
        })
    }

    // Create a fresh queue getting messages from the given source.
    //
    // # Arguments
    //
    // * `id` - A [`QueueId`] that will be used to lookup the queue.
    //
    // # Returns
    //
    // Either a [`Queue`] object, which allows the messages to be read, and containing the ID, or
    // an [`Error`] that gives the reason the queue failed to be created.
    pub async fn open_queue(&self, id: &QueueId) -> Result<Queue, Error> {
        let channel = self.create_channel().await?;
        let queue_name = id.queue_name();
        let consumer = channel
            .basic_consume(&queue_name, "", Default::default(), Default::default())
            .await
            .map_err(Error::new_other)?;
        Ok(Queue {
            id: id.clone(),
            consumer,
        })
    }

    pub async fn send_message(&self, source: &MessageSource, msg: Message) -> Result<(), Error> {
        let channel = self.create_channel().await?;
        let routing_key = source.to_routing_key();

        channel
            .basic_publish(
                PRIMARY_EXCHANGE,
                &routing_key,
                Default::default(),
                msg.0,
                Default::default(),
            )
            .await
            .map_err(Error::new_other)?
            .await
            .map_err(Error::new_other)?;

        Ok(())
    }
}
