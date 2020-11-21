use futures::{channel::mpsc, prelude::*};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message as WsMessage;

use minibot_common::{
    future::{cancel::CancelToken, pipe, try_stream_pipe},
    net::rpc::{ClientChannel, CommandError, CommandHandler},
};

struct ChannelHandler {
    user_id: u64,
}

#[derive(Serialize, Deserialize)]
struct UserIdResponse {
    user_id: u64,
}

impl CommandHandler for ChannelHandler {
    fn start_command(
        &mut self,
        method: &str,
        _payload: &serde_json::Value,
        mut output: mpsc::Sender<serde_json::Value>,
        _cancel: CancelToken,
    ) -> Result<(), CommandError> {
        match method {
            "user_id" => {
                let user_id = self.user_id;
                tokio::spawn(async move {
                    output
                        .send(serde_json::to_value(UserIdResponse { user_id }).unwrap())
                        .await
                        .unwrap();
                });

                Ok(())
            }

            _ => Err(CommandError::UnknownMethod),
        }
    }
}

pub struct ChannelAcceptor {
    /// A mapping from user ids to available client channels.
    channels: std::sync::Mutex<std::collections::HashMap<u64, Vec<ClientChannel>>>,
}

impl ChannelAcceptor {
    pub fn accept<T>(&self, user_id: u64, conn: T) -> anyhow::Result<()>
    where
        T: Stream<Item = WsMessage> + Sink<WsMessage> + Send + 'static,
        <T as Sink<WsMessage>>::Error: Send,
    {
        let (output_ws_msg_start, input_ws_msg_end) = conn.split();

        // We need a cloneable output for ws_messages, to allow for ping/pong handling
        let (split_output_ws_msg_start, split_output_ws_msg_end) = mpsc::channel(0);

        let (input_str_start, input_str_end) = mpsc::channel(0);
        let (output_str_start, output_str_end) = mpsc::channel(0);

        let pong_start = split_output_ws_msg_start.clone();

        let filter_fn = move |item| {
            {
                let mut pong_start = pong_start.clone();
                async move {
                    match item {
                        WsMessage::Text(str) => Some(Ok(str)),
                        WsMessage::Binary(_) => {
                            Some(Err(anyhow::anyhow!("Unexpected binary message.")))
                        }
                        WsMessage::Ping(v) => match pong_start.send(WsMessage::Pong(v)).await {
                            Ok(()) => None,
                            Err(e) => Some(Err(e.into())),
                        },
                        // We don't send pings at the moment, so we don't expect pongs.
                        WsMessage::Pong(_) => None,
                        WsMessage::Close(e) => Some(Err(anyhow::anyhow!("Socket closed: {:?}", e))),
                    }
                }
                .boxed()
            }
        };

        let input_ws_msg_end = input_ws_msg_end.filter_map(filter_fn).boxed();

        let client =
            ClientChannel::new_channel(input_str_end, output_str_start, ChannelHandler { user_id });

        tokio::spawn(async move {
            let (_, _, _) = futures::join!(
                pipe(split_output_ws_msg_end, output_ws_msg_start),
                try_stream_pipe(input_ws_msg_end, input_str_start),
                pipe(
                    output_str_end.map(WsMessage::Text),
                    split_output_ws_msg_start
                )
            );
        });

        let mut guard = self.channels.lock().unwrap();

        guard.entry(user_id).or_insert_with(Vec::new).push(client);

        Ok(())
    }
}
