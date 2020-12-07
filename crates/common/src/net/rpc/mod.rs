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

mod broker;
mod msg;

use futures::channel::mpsc;
use futures::prelude::*;
use msg::Message;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::future::{cancel::CancelToken, deser_json_pipe, pipe, pipe::PipeEnd, ser_json_pipe};

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("Unknown method")]
    UnknownMethod,
}

#[derive(thiserror::Error, Debug)]
pub enum ChannelError {
    #[error("Error while serde JSON: {0}")]
    SerdeError(#[from] serde_json::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum SendError {
    #[error("Channel was closed")]
    SerdeError(#[from] mpsc::SendError),
}

#[derive(thiserror::Error, Debug)]
pub enum SendCommandError {
    #[error("Failed to serialize command")]
    Serde(#[from] serde_json::Error),

    #[error("Channel was closed.")]
    ChannelClosed(#[from] SendError),
}

/// A object-safe trait which can handle incomming commands, and produce a stream of outputs.
pub trait CommandHandler: Send {
    fn start_command(
        &mut self,
        method: &str,
        payload: &serde_json::Value,
        output: mpsc::Sender<serde_json::Value>,
        cancel: CancelToken,
    ) -> Result<(), CommandError>;
}

pub trait Command: Serialize {
    type Response: DeserializeOwned + Send + 'static;
    fn method() -> &'static str;
}

#[derive(Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[serde(transparent)]
pub struct Id(std::num::NonZeroU32);

pub struct ClientChannel {
    event_send: mpsc::Sender<broker::Event>,
}

impl Drop for ClientChannel {
    fn drop(&mut self) {
        let mut event_send = self.event_send.clone();
        tokio::spawn(async move {
            let _ = event_send.send(broker::Event::new_terminate()).await;
        });
    }
}

impl ClientChannel {
    pub fn new_channel<In, Out, H>(
        input_string_end: In,
        output_string_start: Out,
        handler: H,
    ) -> Self
    where
        In: Stream + Unpin + Send + 'static,
        In::Item: std::borrow::Borrow<str> + Send,
        Out: Sink<String> + Unpin + Send + 'static,
        Out::Error: Send,
        H: CommandHandler + 'static,
    {
        let (input_msg_start, input_msg_end) = mpsc::channel(0);
        let (output_msg_start, output_msg_end) = mpsc::channel(0);

        let client = ClientChannel::new_message_channel(input_msg_end, output_msg_start, handler);

        tokio::spawn(async move {
            let _ = futures::join!(
                deser_json_pipe(input_string_end, input_msg_start),
                ser_json_pipe(output_msg_end, output_string_start),
            );
        });

        client
    }

    pub fn new_message_channel<In, Out, H>(stream: In, sink: Out, handler: H) -> Self
    where
        In: Stream<Item = Message> + Unpin + Send + 'static,
        Out: Sink<Message> + Unpin + Send + 'static,
        Out::Error: Send,
        H: CommandHandler + 'static,
    {
        // mpsc channel for output
        let (send, recv) = mpsc::channel(0);

        let (event_send, event_recv) = mpsc::channel(0);

        tokio::spawn({
            let event_send = event_send.clone();
            async move {
                let (_, _, _) = futures::join!(
                    pipe(recv, sink),
                    pipe(stream.map(broker::Event::new_message), event_send.clone()),
                    async move {
                        let mut broker = broker::Broker::new(handler);
                        broker.start(event_recv, send).await
                    }
                );
            }
        });

        ClientChannel { event_send }
    }

    /// Sends a command to the remote end of the connection.
    async fn send_raw_command(
        &mut self,
        method: &str,
        payload: serde_json::Value,
        sink: mpsc::Sender<serde_json::Value>,
    ) -> Result<(), SendError> {
        self.event_send
            .send(broker::Event::new_command(
                method.to_string(),
                payload,
                sink,
            ))
            .await?;

        Ok(())
    }
    pub async fn send_command<Cmd>(
        &mut self,
        command: Cmd,
    ) -> Result<PipeEnd<Cmd::Response>, SendCommandError>
    where
        Cmd: Command,
    {
        let (resp_start, resp_end) = mpsc::channel(0);
        self.send_raw_command(Cmd::method(), serde_json::to_value(&command)?, resp_start)
            .await?;

        Ok(PipeEnd::wrap(resp_end)
            .map(|item| serde_json::from_value(item))
            .end_on_error())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::net::rpc::Command;
    use serde_json::json;

    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    #[serde(transparent)]
    struct EchoCommand(serde_json::Value);

    impl Command for EchoCommand {
        type Response = EchoCommand;

        fn method() -> &'static str {
            "echo"
        }
    }

    struct EchoHandler;

    impl CommandHandler for EchoHandler {
        fn start_command(
            &mut self,
            method: &str,
            payload: &serde_json::Value,
            mut output: mpsc::Sender<serde_json::Value>,
            cancel: CancelToken,
        ) -> Result<(), CommandError> {
            match method {
                "echo" => {
                    let payload = payload.clone();
                    tokio::spawn(async move {
                        cancel
                            .with_canceled_or_else(Ok(()), output.send(payload))
                            .await
                            .unwrap();
                    });

                    Ok(())
                }

                _ => Err(CommandError::UnknownMethod),
            }
        }
    }

    struct NullHandler;

    impl CommandHandler for NullHandler {
        fn start_command(
            &mut self,
            _method: &str,
            _payload: &serde_json::Value,
            _output: mpsc::Sender<serde_json::Value>,
            _cancel: CancelToken,
        ) -> Result<(), CommandError> {
            Err(CommandError::UnknownMethod)
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
        let (sender, in_stream) = mpsc::channel(0);
        let (out_sink, receiver) = mpsc::channel(0);

        let client_channel = ClientChannel::new_message_channel(in_stream, out_sink, handler);

        (client_channel, sender, receiver)
    }

    fn make_test_channel_pair<H1, H2>(handler1: H1, handler2: H2) -> (ClientChannel, ClientChannel)
    where
        H1: CommandHandler + Send + 'static,
        H2: CommandHandler + Send + 'static,
    {
        let (sender, in_stream) = mpsc::channel(0);
        let (out_sink, receiver) = mpsc::channel(0);

        let client_channel_1 = ClientChannel::new_message_channel(in_stream, out_sink, handler1);
        let client_channel_2 = ClientChannel::new_message_channel(receiver, sender, handler2);

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
        send.send(Message::Command(msg::CommandMessage {
            id: Id(std::num::NonZeroU32::new(1).unwrap()),
            method: "echo".to_string(),
            payload: payload_value.clone(),
        }))
        .await
        .unwrap();

        assert_eq!(
            recv.next().await.unwrap(),
            Message::Response(msg::ResponseMessage {
                id: Id(std::num::NonZeroU32::new(1).unwrap()),
                payload: payload_value,
            })
        );
        assert_eq!(
            recv.next().await.unwrap(),
            Message::End(msg::EndMessage {
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

        let resp_stream = chan2
            .send_command(EchoCommand(payload_value.clone()))
            .await?;

        let resps = resp_stream.into_stream().collect::<Vec<_>>().await;

        assert_eq!(resps, vec![EchoCommand(payload_value)]);

        Ok(())
    }
}
