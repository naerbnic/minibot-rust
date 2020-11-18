//! Implementation for the WebSocket channel protocol
//!
//! A channel handles a bidirectional stream of messages that consist of multiplexed commands and
//! streams of responses. A stream can handle multiple commands and responses at once.
//!
//! A single command from A to B consists of the following steps:
//!
//! 1. A sends a "cmd" message to B
//!
//!    This command message consists of an ID chosen by A to be free (i.e. not being used for any
//!    unterminated command started by A), a method string defining which method should be called,
//!    and a payload, consisting of a free JSON value that acts as parameters to the method. Once
//!    this message is sent, the ID is considered allocated.
//!
//!    Note that IDs created by A and B are independent of each other, so there is no possibility
//!    of collision
//!
//! 2. B sends zero or more "resp" messages to A
//!
//!    A resp (response) message consists of an ID indicating which command this is in response to,
//!    and a payload JSON value that counts as the content of the response. The number of response
//!    messages sent back is arbitrary, and is part of the method handler implementation.
//!
//! 3. B sends an "end" message to A
//!
//!    An end message consists only of an ID indicating which command this is ending. Once B sends
//!    an end message, that ID is considered free once again. Thus when A recieves an end message,
//!    they are free to reuse that ID for a future command.
//!
//! It is also possible for A to stop a stream early for B by sending a "cancel" command with
//! the command ID they want to stop receiving responses for. Sending a cancel with an ID does
//! _not_ free the ID. When B recieves a cancel, it SHOULD end the stream at the earliest
//! opportunity, being sure to send an "end" message to indicate stream termination. Sending a
//! cancel message is advisory, and it's up to the method implementor to define how it is handled.
//!
//! The cancel can be part of the protocol of a method. For example, if a method sends back a stream
//! of live data updates, it does not need to send an end message until the stream is cancelled.
//!
//! There may be higher-level protocols built off of this one, such as reserved method names that
//! define some meta-level operations (like querying what methods are available, etc.).
//!
//! TODO: What about needing to terminate and rejoin a session, to switch servers for example? Is
//! there a way to recreate a stream setting, or should that be part of the layer above this one?

use futures::channel::mpsc;
use futures::future::BoxFuture;
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::util::future::{
    cancel::{cancel_pair, CancelHandle, CancelToken},
    deser_json_pipe, pipe, ser_json_pipe,
};

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("Received unknown method: {0:?}")]
    BadMethod(String),
}

#[derive(thiserror::Error, Debug)]
pub enum ChannelError {
    #[error("Error while serde JSON: {0}")]
    SerdeError(#[from] serde_json::Error),
}

/// A object-safe trait which can handle incomming commands, and produce a stream of outputs.
pub trait CommandHandler: Send {
    fn start_command(
        &mut self,
        method: &str,
        payload: &serde_json::Value,
        output: mpsc::Sender<serde_json::Value>,
        cancel: CancelToken,
    ) -> Result<Pin<Box<dyn Future<Output = ()> + Send + 'static>>, CommandError>;
}

#[derive(Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[serde(transparent)]
pub struct Id(std::num::NonZeroU32);

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct CommandMessage {
    id: Id,
    method: String,
    payload: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct CancelMessage {
    id: Id,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct ResponseMessage {
    id: Id,
    payload: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct EndMessage {
    id: Id,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct ErrorMessage {
    error: String,
    id: Option<Id>,
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

struct StreamState {
    cancel_handle: CancelHandle,
}

pub struct ClientChannel {
    event_send: mpsc::Sender<Event>,
}

impl ClientChannel {
    pub fn new_channel<In, Out, H>(
        input_string_end: In,
        output_string_start: Out,
        handler: H,
    ) -> (Self, BoxFuture<'static, ()>)
    where
        In: Stream + Unpin + Send + 'static,
        In::Item: std::borrow::Borrow<str> + Send,
        Out: Sink<String> + Unpin + Send + 'static,
        Out::Error: Send,
        H: CommandHandler + 'static,
    {
        let (input_msg_start, input_msg_end) = mpsc::channel(0);
        let (output_msg_start, output_msg_end) = mpsc::channel(0);

        let (client, fut) =
            ClientChannel::new_message_channel(input_msg_end, output_msg_start, handler);

        (
            client,
            async move {
                let _ = futures::join!(
                    fut,
                    deser_json_pipe(input_string_end, input_msg_start),
                    ser_json_pipe(output_msg_end, output_string_start),
                );
            }
            .boxed(),
        )
    }

    pub fn new_message_channel<In, Out, H>(
        stream: In,
        sink: Out,
        handler: H,
    ) -> (Self, BoxFuture<'static, Result<(), ChannelError>>)
    where
        In: Stream<Item = Message> + Unpin + Send + 'static,
        Out: Sink<Message> + Unpin + Send + 'static,
        Out::Error: Send,
        H: CommandHandler + 'static,
    {
        // mpsc channel for output
        let (send, recv) = mpsc::channel(0);

        let (event_send, event_recv) = mpsc::channel(10);

        let fut = {
            let event_send = event_send.clone();
            async move {
                let (_, _, _) = futures::join!(
                    pipe(recv, sink),
                    pipe(stream.map(Event::Message), event_send.clone(),),
                    async move {
                        let mut broker = Broker::new(handler);
                        broker.start(event_recv, send).await
                    }
                );

                Ok(())
            }
            .boxed()
        };

        (ClientChannel { event_send }, fut)
    }

    pub fn start_message_channel<In, Out, H>(
        stream: In,
        sink: Out,
        handler: H,
    ) -> Self
    where
        In: Stream<Item = Message> + Unpin + Send + 'static,
        Out: Sink<Message> + Unpin + Send + 'static,
        Out::Error: Send,
        H: CommandHandler + 'static,
    {
        let (client, fut) = ClientChannel::new_message_channel(stream, sink, handler);
        tokio::spawn(fut);
        client
    }

    pub async fn send_command(
        &mut self,
        method: &str,
        payload: serde_json::Value,
        sink: mpsc::Sender<serde_json::Value>,
    ) -> anyhow::Result<()> {
        let cmd_event = Event::StartCommand(StartCommandEvent {
            method: method.to_string(),
            payload,
            sink,
        });

        self.event_send.send(cmd_event).await?;

        Ok(())
    }
}

async fn stream_sender_loop(
    id: Id,
    mut client_recv: mpsc::Receiver<serde_json::Value>,
    mut send: mpsc::Sender<Message>,
) -> Result<(), mpsc::SendError> {
    while let Some(msg) = client_recv.next().await {
        send.send(Message::Response(ResponseMessage { id, payload: msg }))
            .await?;
    }

    send.send(Message::End(EndMessage { id })).await?;

    Ok(())
}

struct StartCommandEvent {
    method: String,
    payload: serde_json::Value,
    sink: mpsc::Sender<serde_json::Value>,
}

enum Event {
    StartCommand(StartCommandEvent),
    Message(Message),
}

struct Broker {
    incoming_streams: HashMap<Id, mpsc::Sender<serde_json::Value>>,
    outgoing_streams: HashMap<Id, StreamState>,
    handler: Box<dyn CommandHandler>,
    next_id: u32,
}

impl Broker {
    pub fn new<H: CommandHandler + 'static>(
        handler: H,
    ) -> Self {
        Broker {
            incoming_streams: HashMap::new(),
            outgoing_streams: HashMap::new(),
            handler: Box::new(handler),
            next_id: 1,
        }
    }

    pub async fn start(
        &mut self,
        mut stream: mpsc::Receiver<Event>,
        mut send: mpsc::Sender<Message>,
    ) {
        while let Some(event) = stream.next().await {
            let result = match event {
                Event::StartCommand(cmd) => self.handle_start_command(cmd, &mut send).await,
                Event::Message(msg) => self.handle_message(msg, &mut send).await,
            };

            if let Err(e) = result {
                // Cancellation propagated by dropping stream
                log::error!("Broker stream error: {}", e);
                return;
            }
        }
    }

    async fn handle_start_command(
        &mut self,
        start_command: StartCommandEvent,
        client_send: &mut mpsc::Sender<Message>,
    ) -> anyhow::Result<()> {
        let new_id = self.take_new_id();
        let cmd_message = CommandMessage {
            id: new_id.clone(),
            method: start_command.method,
            payload: start_command.payload,
        };

        client_send.send(Message::Command(cmd_message)).await?;

        self.incoming_streams.insert(new_id, start_command.sink);

        Ok(())
    }

    async fn handle_message(
        &mut self,
        msg: Message,
        send: &mut mpsc::Sender<Message>,
    ) -> anyhow::Result<()> {
        match msg {
            Message::Command(cmd) => {
                if self.outgoing_streams.contains_key(&cmd.id) {
                    send.send(Message::Error(ErrorMessage {
                        error: format!("Started an already running command with id {:?}", cmd.id),
                        id: Some(cmd.id.clone()),
                    }))
                    .await
                    .unwrap();
                } else {
                    let (server_send, client_recv) = mpsc::channel(10);
                    let (cancel_handle, cancel_token) = cancel_pair();

                    let cmd_future = match self.handler.start_command(
                        &cmd.method,
                        &cmd.payload,
                        server_send,
                        cancel_token,
                    ) {
                        Ok(cmd_future) => cmd_future,
                        Err(e) => {
                            send.send(Message::Error(ErrorMessage {
                                error: format!("Error with command: {}", e),
                                id: Some(cmd.id.clone()),
                            }))
                            .await
                            .unwrap();
                            return Err(e.into());
                        }
                    };

                    tokio::spawn(cmd_future);

                    self.outgoing_streams
                        .insert(cmd.id.clone(), StreamState { cancel_handle });

                    // Spawn the future that wraps server outputs, and follows it with an
                    // endstream message
                    tokio::spawn(stream_sender_loop(
                        cmd.id.clone(),
                        client_recv,
                        send.clone(),
                    ));
                }
            }
            Message::Cancel(cancel) => match self.outgoing_streams.remove(&cancel.id) {
                Some(_) => {}
                None => {
                    // Do nothing. It's possible that a cancel reaches the server after it has
                    // sent a stream end, so this would have removed the entry. It's the sender's
                    // responsibility not to reuse an ID until it has seen an end message.
                }
            },
            Message::Response(stream_msg) => match self.incoming_streams.get_mut(&stream_msg.id) {
                Some(sink) => {
                    sink.send(stream_msg.payload).await.unwrap();
                    // FIXME: An error here means that the client has closed. We should cancel
                    // this id, and leave a placeholder to wait for a stream end message.
                }

                None => {
                    send.send(Message::Error(ErrorMessage {
                        id: Some(stream_msg.id.clone()),
                        error: format!("Got a stream message to an unallocated id."),
                    }))
                    .await?;
                    anyhow::bail!("Stream protocol error");
                }
            },
            Message::End(end) => {
                match self.incoming_streams.remove(&end.id) {
                    Some(_) => {
                        // Just let the value drop. It should cause the stream to terminate.
                    }

                    None => {
                        send.send(Message::Error(ErrorMessage {
                            id: Some(end.id.clone()),
                            error: format!("Got a stream message to an unallocated id."),
                        }))
                        .await?;
                        anyhow::bail!("Stream protocol error");
                    }
                }
            }

            Message::Error(err) => {
                // This should terminate the connection.
                anyhow::bail!(
                    "Stream error from peer: id: {:?}, error: {}",
                    err.id,
                    err.error
                );
            }
        }

        Ok(())
    }

    fn take_new_id(&mut self) -> Id {
        let new_id = self.next_id;
        if self.next_id == u32::max_value() {
            self.next_id = 1
        }
        Id(std::num::NonZeroU32::new(new_id).unwrap())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[derive(Clone)]
    struct TokioSpawner;

    struct EchoHandler;

    impl CommandHandler for EchoHandler {
        fn start_command(
            &mut self,
            method: &str,
            payload: &serde_json::Value,
            mut output: mpsc::Sender<serde_json::Value>,
            mut cancel: CancelToken,
        ) -> Result<Pin<Box<dyn Future<Output = ()> + Send + 'static>>, CommandError> {
            match method {
                "echo" => {
                    let payload = payload.clone();
                    Ok(async move {
                        cancel
                            .with_cancelled_default(Ok(()), output.send(payload))
                            .await
                            .unwrap();
                    }
                    .boxed())
                }

                _ => Err(CommandError::BadMethod(method.to_string())),
            }
        }
    }

    struct NullHandler;

    impl CommandHandler for NullHandler {
        fn start_command(
            &mut self,
            method: &str,
            _payload: &serde_json::Value,
            _output: mpsc::Sender<serde_json::Value>,
            _cancel: CancelToken,
        ) -> Result<Pin<Box<dyn Future<Output = ()> + Send + 'static>>, CommandError> {
            Err(CommandError::BadMethod(method.to_string()))
        }
    }

    fn make_test_channel<H>(
        handler: H,
    ) -> (
        ClientChannel,
        mpsc::Sender<Message>,
        mpsc::Receiver<Message>,
    )
    where
        H: CommandHandler + Send + 'static,
    {
        let (sender, in_stream) = mpsc::channel(10);
        let (out_sink, receiver) = mpsc::channel(10);

        let client_channel =
            ClientChannel::start_message_channel(in_stream, out_sink, handler);

        (client_channel, sender, receiver)
    }

    fn make_test_channel_pair<H1, H2>(handler1: H1, handler2: H2) -> (ClientChannel, ClientChannel)
    where
        H1: CommandHandler + Send + 'static,
        H2: CommandHandler + Send + 'static,
    {
        let (sender, in_stream) = mpsc::channel(10);
        let (out_sink, receiver) = mpsc::channel(10);

        let client_channel_1 =
            ClientChannel::start_message_channel(in_stream, out_sink, handler1);
        let client_channel_2 =
            ClientChannel::start_message_channel(receiver, sender, handler2);

        (client_channel_1, client_channel_2)
    }

    #[tokio::test]
    async fn raw_simple_test() {
        let (_chan, mut send, mut recv) = make_test_channel(EchoHandler);
        let payload_value = json!(
            {
                "field1": 1,
                "field2": "Hello, World!\n",
            }
        );
        send.send(Message::Command(CommandMessage {
            id: Id(std::num::NonZeroU32::new(1).unwrap()),
            method: "echo".to_string(),
            payload: payload_value.clone(),
        }))
        .await
        .unwrap();

        assert_eq!(
            recv.next().await.unwrap(),
            Message::Response(ResponseMessage {
                id: Id(std::num::NonZeroU32::new(1).unwrap()),
                payload: payload_value,
            })
        );
        assert_eq!(
            recv.next().await.unwrap(),
            Message::End(EndMessage {
                id: Id(std::num::NonZeroU32::new(1).unwrap()),
            })
        );
    }

    #[tokio::test]
    async fn simple_test() -> anyhow::Result<()> {
        let (_chan1, mut chan2) = make_test_channel_pair(EchoHandler, NullHandler);
        let payload_value = json!(
            {
                "field1": 1,
                "field2": "Hello, World!\n",
            }
        );

        let (sink, resp_stream) = mpsc::channel(10);

        chan2
            .send_command("echo", payload_value.clone(), sink)
            .await?;

        let resps = resp_stream.collect::<Vec<_>>().await;

        assert_eq!(resps, vec![payload_value]);

        Ok(())
    }
}
