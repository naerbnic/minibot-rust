use futures::channel::mpsc;
use futures::prelude::*;
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
pub struct SquelchMessage {
    id: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    id: String,
    payload: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EndStreamMessage {
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
    #[serde(rename = "squelch")]
    Squelch(SquelchMessage),
    #[serde(rename = "stream")]
    Stream(StreamMessage),
    #[serde(rename = "endstream")]
    EndStream(EndStreamMessage),
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
    pub fn start_channel<In, H>(stream: In, sink: mpsc::Sender<Message>, handler: H) -> Self
    where
        In: Stream<Item = Message> + Send + Unpin + 'static,
        H: CommandHandler + 'static,
    {
        let (new_chan, fut) = ClientChannel::new_channel(stream, sink, handler);
        tokio::spawn(fut);
        new_chan
    }

    pub fn new_channel<In, H>(
        stream: In,
        sink: mpsc::Sender<Message>,
        handler: H,
    ) -> (Self, impl Future<Output = ()>)
    where
        In: Stream<Item = Message> + Send + Unpin + 'static,
        H: CommandHandler + 'static,
    {
        // mpsc channel for output
        let (send, recv) = mpsc::channel(10);

        let (event_send, event_recv) = mpsc::channel(10);

        let future = {
            let event_send = event_send.clone();
            async move {
                futures::join!(
                    join_channel_ends(recv, sink),
                    join_channel_ends(stream.map(Event::Message), event_send.clone(),),
                    async move {
                        let mut broker = Broker::new(handler);
                        broker.start(event_recv, send).await
                    }
                );
            }
        };

        (ClientChannel { event_send }, future)
    }

    pub fn send_command(
        &self,
        _method: &str,
        _payload: serde_json::Value,
        _sink: mpsc::Sender<serde_json::Value>,
    ) -> anyhow::Result<()> {
        todo!()
    }
}

async fn stream_sender_loop(
    id: String,
    mut client_recv: mpsc::Receiver<serde_json::Value>,
    mut send: mpsc::Sender<Message>,
) -> anyhow::Result<()> {
    while let Some(msg) = client_recv.next().await {
        send.send(Message::Stream(StreamMessage {
            id: id.clone(),
            payload: msg,
        }))
        .await?;
    }

    send.send(Message::EndStream(EndStreamMessage { id }))
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
    handler: Box<dyn CommandHandler + 'static>,
    next_id: u32,
}

impl Broker {
    pub fn new<H: CommandHandler + 'static>(handler: H) -> Self {
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
                    tokio::spawn(stream_sender_loop(
                        cmd.id.clone(),
                        client_recv,
                        send.clone(),
                    ));
                }
            }
            Message::Squelch(squelch) => match self.outgoing_streams.remove(&squelch.id) {
                Some(_) => {}
                None => todo!("Handle non-existent stream for squelch"),
            },
            Message::Stream(stream_msg) => match self.incoming_streams.get_mut(&stream_msg.id) {
                Some(sink) => {
                    let _ = sink.send(stream_msg.payload).await;
                }

                None => {
                    let _ = send.send(Message::Error(ErrorMessage {
                        id: Some(stream_msg.id.clone()),
                        error: format!("Got a stream message to an unallocated id."),
                    }));
                }
            },
            Message::EndStream(end) => {
                match self.incoming_streams.remove(&end.id) {
                    Some(_) => {
                        // Just let the value drop. It should cause the stream to terminate.
                    }

                    None => {
                        let _ = send.send(Message::Error(ErrorMessage {
                            id: Some(end.id.clone()),
                            error: format!("Got a stream message to an unallocated id."),
                        }));
                    }
                }
            }

            Message::Error(err) => {
                // This should terminate the connection.
                anyhow::bail!("Got stream error: id: {:?}, error: {}", err.id, err.error);
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
