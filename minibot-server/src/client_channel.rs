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
//! cancel message can disregard the internal protocol for the response stream, leaving it
//! incomplete. What an early cancellation means for these methods is up to the method implementor.
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
use futures::prelude::*;
use futures::task::{Spawn, SpawnExt as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::util::cancel::{cancel_pair, CancelHandle, CancelToken};

/// A object-safe trait which can handle incomming commands, and produce a stream of outputs.
pub trait CommandHandler: Send {
    fn start_command(
        &mut self,
        method: &str,
        payload: &serde_json::Value,
        output: mpsc::Sender<serde_json::Value>,
        cancel: CancelToken,
    );
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CommandMessage {
    id: String,
    method: String,
    payload: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CancelMessage {
    id: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    id: String,
    payload: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EndMessage {
    id: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    error: String,
    id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
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
    pub fn start_channel<In, H, S>(
        stream: In,
        sink: mpsc::Sender<Message>,
        spawner: S,
        handler: H,
    ) -> Self
    where
        In: Stream<Item = Message> + Send + Unpin + 'static,
        H: CommandHandler + 'static,
        S: Spawn + Send + Clone + 'static,
    {
        // mpsc channel for output
        let (send, recv) = mpsc::channel(10);

        let (event_send, event_recv) = mpsc::channel(10);

        let inner_spawner = spawner.clone();

        spawner
            .spawn({
                let event_send = event_send.clone();
                async move {
                    futures::join!(
                        join_channel_ends(recv, sink),
                        join_channel_ends(stream.map(Event::Message), event_send.clone(),),
                        async move {
                            let mut broker = Broker::new(handler, inner_spawner);
                            broker.start(event_recv, send).await
                        }
                    );
                }
            })
            .unwrap();

        ClientChannel {
            event_send
        }
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
    id: String,
    mut client_recv: mpsc::Receiver<serde_json::Value>,
    mut send: mpsc::Sender<Message>,
) -> Result<(), mpsc::SendError> {
    while let Some(msg) = client_recv.next().await {
        send.send(Message::Response(ResponseMessage {
            id: id.clone(),
            payload: msg,
        }))
        .await?;
    }

    send.send(Message::End(EndMessage { id }))
        .await?;

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
    incoming_streams: HashMap<String, mpsc::Sender<serde_json::Value>>,
    outgoing_streams: HashMap<String, StreamState>,
    handler: Box<dyn CommandHandler>,
    spawner: Box<dyn Spawn + Send>,
    next_id: u32,
}

impl Broker {
    pub fn new<H: CommandHandler + 'static, S: Spawn + Send + 'static>(
        handler: H,
        spawner: S,
    ) -> Self {
        Broker {
            incoming_streams: HashMap::new(),
            outgoing_streams: HashMap::new(),
            handler: Box::new(handler),
            spawner: Box::new(spawner),
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
                        error: format!("Started an already running command with id {}", cmd.id),
                        id: Some(cmd.id.clone()),
                    }))
                    .await
                    .unwrap();
                } else {
                    let (server_send, client_recv) = mpsc::channel(10);
                    let (cancel_handle, cancel_token) = cancel_pair();

                    self.handler.start_command(
                        &cmd.method,
                        &cmd.payload,
                        server_send,
                        cancel_token,
                    );

                    self.outgoing_streams
                        .insert(cmd.id.clone(), StreamState { cancel_handle });

                    // Spawn the future that wraps server outputs, and follows it with an
                    // endstream message
                    self.spawner.spawn(
                        stream_sender_loop(cmd.id.clone(), client_recv, send.clone()).map(|_| ()),
                    ).expect("The executor must be running for us to get here");
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
                    })).await?;
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
                        })).await?;
                        anyhow::bail!("Stream protocol error");
                    }
                }
            }

            Message::Error(err) => {
                // This should terminate the connection.
                anyhow::bail!("Stream error from peer: id: {:?}, error: {}", err.id, err.error);
            }
        }

        Ok(())
    }

    fn take_new_id(&mut self) -> String {
        let new_id = self.next_id;
        if self.next_id == u32::max_value() {
            self.next_id = 1
        }
        new_id.to_string()
    }
}

async fn join_channel_ends<T, In>(mut recv: In, mut send: mpsc::Sender<T>)
where
    In: Stream<Item = T> + Unpin,
{
    while let Some(v) = recv.next().await {
        // The only error we can get from a Sender is that the stream was disconnected.
        // By dropping the stream, we either cancel it, or propagate an error up the
        // chain.
        if let Err(_) = send.send(v).await {
            return;
        }
    }
}
