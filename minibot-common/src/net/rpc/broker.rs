use std::collections::HashMap;

use futures::channel::mpsc;
use futures::prelude::*;

use crate::future::cancel::{cancel_pair, CancelHandle};

use super::msg::{self, Message};
use super::CommandHandler;
use super::Id;

struct StartCommandEvent {
    method: String,
    payload: serde_json::Value,
    sink: mpsc::Sender<serde_json::Value>,
}

enum Contents {
    StartCommand(StartCommandEvent),
    Terminate,
    Message(Message),
}

pub struct Event(Contents);

impl Event {
    pub fn new_command(
        method: String,
        payload: serde_json::Value,
        sink: mpsc::Sender<serde_json::Value>,
    ) -> Event {
        Event(Contents::StartCommand(StartCommandEvent {
            method,
            payload,
            sink,
        }))
    }
    pub fn new_message(message: Message) -> Event {
        Event(Contents::Message(message))
    }

    pub fn new_terminate() -> Event {
        Event(Contents::Terminate)
    }
}

struct StreamState {
    #[allow(dead_code)]
    cancel_handle: CancelHandle,
}

async fn stream_sender_loop(
    id: Id,
    mut client_recv: mpsc::Receiver<serde_json::Value>,
    mut send: mpsc::Sender<Message>,
) -> Result<(), mpsc::SendError> {
    while let Some(msg) = client_recv.next().await {
        send.send(Message::Response(msg::ResponseMessage { id, payload: msg }))
            .await?;
    }

    send.send(Message::End(msg::EndMessage { id })).await?;

    Ok(())
}

pub struct Broker {
    incoming_streams: HashMap<Id, mpsc::Sender<serde_json::Value>>,
    outgoing_streams: HashMap<Id, StreamState>,
    handler: Box<dyn CommandHandler>,
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
        while let Some(Event(contents)) = stream.next().await {
            let result = match contents {
                Contents::StartCommand(cmd) => self.handle_start_command(cmd, &mut send).await,
                Contents::Message(msg) => self.handle_message(msg, &mut send).await,
                Contents::Terminate => todo!(),
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
        let cmd_message = msg::CommandMessage {
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
                    send.send(Message::new_error_with_id(
                        cmd.id.clone(),
                        "Started an already running command",
                    ))
                    .await
                    .unwrap();
                } else {
                    let (server_send, client_recv) = mpsc::channel(10);
                    let (cancel_handle, cancel_token) = cancel_pair();

                    if let Err(e) = self.handler.start_command(
                        &cmd.method,
                        &cmd.payload,
                        server_send,
                        cancel_token,
                    ) {
                        send.send(Message::Error(msg::ErrorMessage {
                            error: format!("Error with command: {}", e),
                            id: Some(cmd.id.clone()),
                        }))
                        .await
                        .unwrap();
                        return Err(e.into());
                    };

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
                    send.send(Message::Error(msg::ErrorMessage {
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
                        send.send(Message::Error(msg::ErrorMessage {
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
                anyhow::bail!("Stream error from peer: id: {:?}", err,);
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
